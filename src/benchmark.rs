use crate::args::ApiType;
use crate::client::{self, Event, Failure};
use crate::tokens;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use reqwest::{Client, Request};
use serde::Serialize;
use serde_json::Value;
use std::sync::LazyLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokenizers::Tokenizer;
use tokio::sync::watch;

pub fn unix_time_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock precedes Unix epoch")
        .as_nanos()
        .try_into()
        .expect("Unix timestamp exceeds u64")
}

static CLOCK: LazyLock<(Instant, u64)> = LazyLock::new(|| (Instant::now(), unix_time_ns()));

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Measurement,
    Warmup,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Completed,
    Failed,
    TimedOut,
    Cancelled,
}

pub struct PreparedRequest {
    pub id: u32,
    pub phase: Phase,
    pub input_target: u32,
    pub input_tokens: u64,
    pub output_cap: Option<u32>,
    pub request: Request,
}

#[derive(Debug, Default, Serialize)]
pub struct Metrics {
    pub request_latency_ms: Option<f64>,
    pub ttft_ms: Option<f64>,
    pub ttfo_ms: Option<f64>,
    pub generation_window_ms: Option<f64>,
    pub generation_tokens_per_second: Option<f64>,
    pub mean_inter_token_latency_ms: Option<f64>,
    pub mean_inter_event_latency_ms: Option<f64>,
    pub max_inter_event_latency_ms: Option<f64>,
    pub input_tokens: u64,
    pub content_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub generated_tokens: Option<u64>,
    pub delivery_events: u64,
}

#[derive(Debug, Serialize)]
pub struct RequestRecord {
    pub request_id: u32,
    pub phase: Phase,
    pub start_time_unix_ns: u64,
    pub end_time_unix_ns: u64,
    pub elapsed_ms: f64,
    pub status: Status,
    pub http_status: Option<u16>,
    pub finish_reason: Option<String>,
    pub input_target_tokens: u32,
    pub output_cap: Option<u32>,
    pub metrics: Metrics,
    pub reasoning_kinds: Vec<&'static str>,
    pub provider_usage: Option<Value>,
    pub error: Option<Failure>,
    #[serde(skip)]
    pub start: Instant,
    #[serde(skip)]
    pub end: Instant,
}

#[derive(Default)]
struct Observation {
    content: String,
    reasoning: String,
    reasoning_kinds: Vec<&'static str>,
    first: Option<Instant>,
    first_content: Option<Instant>,
    last: Option<Instant>,
    events: u64,
    max_gap: Duration,
    usage: Option<Value>,
    finish_reason: Option<String>,
    http_status: Option<u16>,
}

fn merge_usage(target: &mut Value, update: &Value) {
    if let (Some(target), Some(update)) = (target.as_object_mut(), update.as_object()) {
        for (key, value) in update {
            if !value.is_null() {
                match target.get_mut(key) {
                    Some(old) if old.is_object() && value.is_object() => merge_usage(old, value),
                    _ => {
                        target.insert(key.clone(), value.clone());
                    }
                }
            }
        }
    }
}

impl Observation {
    fn accept(&mut self, event: &Event<'_>, now: Instant) {
        if !event.content.is_empty() || !event.reasoning.is_empty() {
            self.first.get_or_insert(now);
            if let Some(previous) = self.last {
                self.max_gap = self.max_gap.max(now.duration_since(previous));
            }
            self.last = Some(now);
            self.events += 1;
        }
        if !event.content.is_empty() {
            self.first_content.get_or_insert(now);
            self.content.push_str(event.content);
        }
        if !event.reasoning.is_empty() {
            self.reasoning.push_str(event.reasoning);
            if let Some(kind) = event.reasoning_kind {
                if !self.reasoning_kinds.contains(&kind) {
                    self.reasoning_kinds.push(kind);
                }
            }
        }
        if let Some(usage) = event.usage {
            let current = self.usage.get_or_insert_with(|| serde_json::json!({}));
            merge_usage(current, usage);
        }
        if let Some(reason) = event.finish_reason {
            self.finish_reason = Some(reason.to_owned());
        }
    }

    fn metrics(
        &self,
        start: Instant,
        end: Instant,
        completed: bool,
        input: u64,
        content: Option<u64>,
        reasoning: Option<u64>,
    ) -> Metrics {
        let window = self
            .first
            .zip(self.last)
            .map(|(a, b)| b.duration_since(a).as_secs_f64());
        let generated = content.zip(reasoning).map(|(a, b)| a + b);
        let per_token = window
            .zip(generated)
            .filter(|(seconds, n)| *seconds > 0.0 && *n > 1)
            .map(|(seconds, n)| seconds / (n - 1) as f64);
        Metrics {
            request_latency_ms: completed.then(|| end.duration_since(start).as_secs_f64() * 1000.0),
            ttft_ms: self
                .first
                .map(|t| t.duration_since(start).as_secs_f64() * 1000.0),
            ttfo_ms: self
                .first_content
                .map(|t| t.duration_since(start).as_secs_f64() * 1000.0),
            generation_window_ms: window.map(|s| s * 1000.0),
            generation_tokens_per_second: per_token.map(|s| 1.0 / s),
            mean_inter_token_latency_ms: per_token.map(|s| s * 1000.0),
            mean_inter_event_latency_ms: window
                .filter(|_| self.events > 1)
                .map(|s| s * 1000.0 / (self.events - 1) as f64),
            max_inter_event_latency_ms: (self.events > 1)
                .then_some(self.max_gap.as_secs_f64() * 1000.0),
            input_tokens: input,
            content_tokens: content,
            reasoning_tokens: reasoning,
            generated_tokens: generated,
            delivery_events: self.events,
        }
    }
}

pub struct Captured {
    record: RequestRecord,
    observation: Observation,
}

impl Captured {
    pub fn finish(mut self, tokenizer: &Tokenizer) -> RequestRecord {
        let content = tokens::count(tokenizer, &self.observation.content);
        let reasoning = tokens::count(tokenizer, &self.observation.reasoning);
        if let Some(error) = content.as_ref().err().or(reasoning.as_ref().err()) {
            self.record.status = Status::Failed;
            self.record.error = Some(Failure::new("tokenization", error));
        }
        self.record.metrics = self.observation.metrics(
            self.record.start,
            self.record.end,
            self.record.status == Status::Completed,
            self.record.metrics.input_tokens,
            content.ok(),
            reasoning.ok(),
        );
        self.record.reasoning_kinds = self.observation.reasoning_kinds;
        self.record.provider_usage = self.observation.usage;
        self.record.finish_reason = self.observation.finish_reason;
        self.record.http_status = self.observation.http_status;
        self.record
    }
}

async fn receive(
    client: &Client,
    api: ApiType,
    request: Request,
    observation: &mut Observation,
) -> Result<(), Failure> {
    let response = client
        .execute(request)
        .await
        .map_err(|e| Failure::new("transport", e.without_url()))?;
    observation.http_status = Some(response.status().as_u16());
    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| Failure::new("transport", e.without_url()))?;
        return Err(Failure::new("http", format!("HTTP {status}: {body}")));
    }
    if !response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.split(';')
                .next()
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("text/event-stream"))
        })
    {
        return Err(Failure::new(
            "protocol",
            "expected Content-Type: text/event-stream",
        ));
    }
    let mut stream = response.bytes_stream().eventsource();
    while let Some(event) = stream.next().await {
        let now = Instant::now();
        let event = event.map_err(|e| Failure::new("stream", e))?;
        if event.data.trim().is_empty() {
            continue;
        }
        if event.data.trim() == "[DONE]" {
            if api == ApiType::Chat {
                return Ok(());
            }
            return Err(Failure::new(
                "protocol",
                "missing API completion event before [DONE]",
            ));
        }
        let value: Value =
            serde_json::from_str(&event.data).map_err(|e| Failure::new("protocol", e))?;
        let event = client::parse_event(api, &value)?;
        observation.accept(&event, now);
        if let Some(failure) = event.failure {
            return Err(failure);
        }
        if event.done {
            return Ok(());
        }
    }
    Err(Failure::new(
        "protocol",
        "stream ended without its completion event",
    ))
}

pub async fn capture(
    client: Client,
    api: ApiType,
    prepared: PreparedRequest,
    timeout: Duration,
    mut cancel: watch::Receiver<bool>,
) -> Captured {
    let (origin, unix_ns) = *CLOCK;
    let start = Instant::now();
    let start_time_unix_ns = unix_ns
        + u64::try_from(start.duration_since(origin).as_nanos())
            .expect("run duration exceeds u64 nanoseconds");
    let mut observation = Observation::default();
    let (status, error) = tokio::select! {
        biased;
        _ = cancel.wait_for(|cancelled| *cancelled) => (Status::Cancelled, Some(Failure::new("cancelled", "interrupted"))),
        result = tokio::time::timeout(timeout, receive(&client, api, prepared.request, &mut observation)) => match result {
            Ok(Ok(())) => (Status::Completed, None),
            Ok(Err(error)) => (Status::Failed, Some(error)),
            Err(_) => (Status::TimedOut, Some(Failure::new("timeout", "request deadline exceeded"))),
        }
    };
    let end = Instant::now();
    let elapsed = end.duration_since(start);
    Captured {
        record: RequestRecord {
            request_id: prepared.id,
            phase: prepared.phase,
            start_time_unix_ns,
            end_time_unix_ns: start_time_unix_ns
                .saturating_add(elapsed.as_nanos().try_into().unwrap_or(u64::MAX)),
            elapsed_ms: elapsed.as_secs_f64() * 1000.0,
            status,
            http_status: None,
            finish_reason: None,
            input_target_tokens: prepared.input_target,
            output_cap: prepared.output_cap,
            metrics: Metrics {
                input_tokens: prepared.input_tokens,
                ..Metrics::default()
            },
            reasoning_kinds: Vec::new(),
            provider_usage: None,
            error,
            start,
            end,
        },
        observation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_excludes_prefill_and_completion_tail_from_generation() {
        let start = Instant::now();
        let mut obs = Observation::default();
        obs.accept(
            &Event {
                reasoning: "a b",
                reasoning_kind: Some("text"),
                ..Event::default()
            },
            start + Duration::from_millis(100),
        );
        obs.accept(
            &Event {
                content: "c d",
                ..Event::default()
            },
            start + Duration::from_millis(300),
        );
        let m = obs.metrics(
            start,
            start + Duration::from_millis(500),
            true,
            8,
            Some(2),
            Some(2),
        );
        assert_eq!(m.ttft_ms, Some(100.0));
        assert_eq!(m.ttfo_ms, Some(300.0));
        assert_eq!(m.request_latency_ms, Some(500.0));
        assert_eq!(m.generation_window_ms, Some(200.0));
        assert_eq!(m.generation_tokens_per_second, Some(15.0));
        assert_eq!(m.mean_inter_event_latency_ms, Some(200.0));
        assert_eq!(m.max_inter_event_latency_ms, Some(200.0));
        assert_eq!(m.generated_tokens, Some(4));
    }

    #[test]
    fn missing_and_single_event_timings_are_not_fabricated() {
        let start = Instant::now();
        let mut obs = Observation::default();
        let m = obs.metrics(start, start, true, 1, Some(0), Some(0));
        assert_eq!(m.ttft_ms, None);
        assert_eq!(m.ttfo_ms, None);
        obs.accept(
            &Event {
                content: "a b",
                reasoning: "c d",
                ..Event::default()
            },
            start,
        );
        let m = obs.metrics(start, start, true, 1, Some(2), Some(2));
        assert_eq!(m.delivery_events, 1);
        assert_eq!(m.generation_window_ms, Some(0.0));
        assert_eq!(m.generation_tokens_per_second, None);
        assert_eq!(m.mean_inter_event_latency_ms, None);
    }

    #[test]
    fn usage_updates_preserve_input_counts_and_never_enter_local_counts() {
        let start = Instant::now();
        let mut obs = Observation::default();
        let first = serde_json::json!({"input_tokens":12,"output_tokens":0});
        let last = serde_json::json!({"output_tokens":100,"output_tokens_details":{"reasoning_tokens":90}});
        obs.accept(
            &Event {
                usage: Some(&first),
                ..Event::default()
            },
            start,
        );
        obs.accept(
            &Event {
                usage: Some(&last),
                ..Event::default()
            },
            start,
        );
        assert_eq!(obs.usage.as_ref().unwrap()["input_tokens"], 12);
        let m = obs.metrics(start, start, true, 1, Some(0), Some(0));
        assert_eq!(m.generated_tokens, Some(0));
        assert_eq!(m.ttft_ms, None);
    }
}
