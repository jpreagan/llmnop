mod screen;
mod view;

use crate::args::{Args, OutputFormat};
use crate::benchmark::{Delta, Phase, RequestRecord, Status};
use crate::output::Counts;
use crate::report;
use crate::style::{group, seconds};
use crate::tokens;
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span, Text};
use screen::Screen;
use std::collections::BTreeMap;
use std::io::{self, IsTerminal};
use std::time::{Duration, Instant};
use tokenizers::Tokenizer;
use tokio::sync::watch;
use tokio::time::{Interval, MissedTickBehavior};

const REFRESH: Duration = Duration::from_millis(100);
const STALL: Duration = Duration::from_secs(10);

pub fn frames() -> Interval {
    let mut interval = tokio::time::interval(REFRESH);
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    interval
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Tokenizer,
    Prompts,
    Run(Phase),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Activity {
    Waiting,
    Reasoning,
    Content,
    Stalled,
}

enum Outcome {
    Done,
    Capped,
    Failed(String),
    TimedOut,
    Cancelled,
}

impl Outcome {
    fn of(record: &RequestRecord) -> Self {
        match record.status {
            Status::Completed if record.stopped_at_output_cap() => Self::Capped,
            Status::Completed => Self::Done,
            Status::Failed => Self::Failed(match (record.http_status, &record.error) {
                (Some(status), _) if status >= 400 => format!("HTTP {status}"),
                (_, Some(error)) => error.category.to_string(),
                _ => "failed".into(),
            }),
            Status::TimedOut => Self::TimedOut,
            Status::Cancelled => Self::Cancelled,
        }
    }

    fn succeeded(&self) -> bool {
        matches!(self, Self::Done | Self::Capped)
    }

    fn failed(&self) -> bool {
        matches!(self, Self::Failed(_) | Self::TimedOut)
    }
}

struct Request {
    started: Instant,
    ended: Option<Instant>,
    first: Option<Instant>,
    last: Option<Instant>,
    activity: Vec<(Instant, Activity)>,
    pending_content: String,
    pending_reasoning: String,
    content: u64,
    reasoning: u64,
    ttft: Option<Duration>,
    rate: Option<f64>,
    outcome: Option<Outcome>,
}

impl Request {
    fn new() -> Self {
        let started = Instant::now();
        Self {
            started,
            ended: None,
            first: None,
            last: None,
            activity: vec![(started, Activity::Waiting)],
            pending_content: String::new(),
            pending_reasoning: String::new(),
            content: 0,
            reasoning: 0,
            ttft: None,
            rate: None,
            outcome: None,
        }
    }

    fn in_flight(&self) -> bool {
        self.outcome.is_none()
    }

    fn mark(&mut self, at: Instant, activity: Activity) {
        if self.activity.last().map(|(_, a)| *a) != Some(activity) {
            self.activity.push((at, activity));
        }
    }

    fn ttft(&self) -> Option<Duration> {
        self.ttft
            .or_else(|| self.first.map(|t| t.duration_since(self.started)))
    }

    fn generated(&self) -> u64 {
        self.content + self.reasoning
    }
}

struct State {
    model: String,
    api: String,
    url: String,
    settings: String,
    tokenizer_name: String,
    prints_report: bool,
    stage: Stage,
    stage_started: Instant,
    total: usize,
    cancel: watch::Receiver<bool>,
    requests: BTreeMap<u32, Request>,
    tally: Counts,
    latency: Vec<f64>,
    buckets: Vec<u64>,
    bucket_at: Instant,
}

pub struct Ui {
    screen: Option<Screen>,
    state: State,
    cancel: watch::Sender<bool>,
    diagnostics: Vec<Line<'static>>,
}

impl Ui {
    pub fn new(args: &Args, cancel: watch::Sender<bool>) -> Self {
        let mut settings = format!(
            "concurrency {} · input {}",
            args.concurrency, args.input_tokens
        );
        if let Some(cap) = args.output_cap {
            settings.push_str(&format!(" · cap {cap}"));
        }
        let state = State {
            model: args.model.clone().unwrap_or_default(),
            api: args.api.to_string(),
            url: args.url.clone().unwrap_or_default(),
            settings,
            tokenizer_name: args.tokenizer.clone().unwrap_or_default(),
            prints_report: matches!(args.format, OutputFormat::Table),
            stage: Stage::Tokenizer,
            stage_started: Instant::now(),
            total: 0,
            cancel: cancel.subscribe(),
            requests: BTreeMap::new(),
            tally: Counts::default(),
            latency: Vec::new(),
            buckets: vec![0],
            bucket_at: Instant::now(),
        };
        let supports_screen = std::env::var_os("TERM")
            .map(|term| !term.is_empty() && term != "dumb")
            .unwrap_or(cfg!(windows));
        let screen = (io::stderr().is_terminal() && supports_screen)
            .then(Screen::enter)
            .and_then(Result::ok);
        Self {
            screen,
            state,
            cancel,
            diagnostics: Vec::new(),
        }
    }

    pub fn interactive(&self) -> bool {
        self.screen.is_some()
    }

    pub async fn attend<T: Send + 'static>(
        &mut self,
        stage: Stage,
        total: usize,
        task: impl FnOnce() -> T + Send + 'static,
    ) -> Option<T> {
        self.begin(stage, total);
        let mut task = tokio::task::spawn_blocking(task);
        let mut tick = frames();
        let mut cancel = self.state.cancel.clone();
        loop {
            tokio::select! {
                result = &mut task => return Some(result.expect("preparation task panicked")),
                _ = cancel.wait_for(|cancelled| *cancelled) => return None,
                _ = tick.tick() => self.draw(),
            }
        }
    }

    pub fn begin(&mut self, stage: Stage, total: usize) {
        let state = &mut self.state;
        let now = Instant::now();
        state.stage = stage;
        state.stage_started = now;
        state.total = total;
        state.requests.clear();
        state.tally = Counts::default();
        state.latency.clear();
        state.buckets = vec![0];
        state.bucket_at = now;
        if self.screen.is_none() {
            eprintln!("{}", state.stage_label());
        }
        self.draw();
    }

    pub fn started(&mut self, id: u32) {
        self.state.requests.insert(id, Request::new());
    }

    pub fn delta(&mut self, delta: Delta) {
        let Some(request) = self
            .state
            .requests
            .get_mut(&delta.id)
            .filter(|r| r.in_flight())
        else {
            return;
        };
        request.pending_content.push_str(&delta.content);
        request.pending_reasoning.push_str(&delta.reasoning);
        if let Some(last) = request.last.filter(|last| delta.at - *last >= STALL) {
            request.mark(last, Activity::Stalled);
        }
        let activity = if delta.content.is_empty() {
            Activity::Reasoning
        } else {
            Activity::Content
        };
        request.mark(delta.at, activity);
        request.first.get_or_insert(delta.at);
        request.last = Some(delta.at);
    }

    pub fn tokenize(&mut self, tokenizer: &Tokenizer) {
        let state = &mut self.state;
        state.advance(Instant::now());
        let mut arrived = 0;
        for request in state.requests.values_mut().filter(|r| r.in_flight()) {
            let content = drain(&mut request.pending_content, tokenizer);
            let reasoning = drain(&mut request.pending_reasoning, tokenizer);
            request.content += content;
            request.reasoning += reasoning;
            arrived += content + reasoning;
        }
        *state.buckets.last_mut().unwrap() += arrived;
    }

    pub fn finished(&mut self, record: &RequestRecord) {
        let state = &mut self.state;
        state.advance(Instant::now());
        if let Some(request) = state
            .requests
            .get_mut(&record.request_id)
            .filter(|r| r.in_flight())
        {
            let m = &record.metrics;
            if let Some(generated) = m.generated_tokens {
                let remaining = generated.saturating_sub(request.generated());
                *state.buckets.last_mut().unwrap() += remaining;
            }
            request.pending_content.clear();
            request.pending_reasoning.clear();
            request.started = record.start;
            request.ended = Some(record.end);
            request.content = m.content_tokens.unwrap_or(request.content);
            request.reasoning = m.reasoning_tokens.unwrap_or(request.reasoning);
            request.ttft = m.ttft_ms.map(|ms| Duration::from_secs_f64(ms / 1000.0));
            request.rate = m.generation_tokens_per_second;
            request.outcome = Some(Outcome::of(record));
        }
        state.tally.add(record);
        if record.status == Status::Completed {
            state.latency.extend(record.metrics.request_latency_ms);
        }
        let line = trail(record);
        if self.screen.is_none() {
            eprintln!("{line}");
        } else if record.status != Status::Completed {
            self.diagnostics.push(line);
        }
    }

    pub fn draw(&mut self) {
        let Some(screen) = self.screen.as_mut() else {
            return;
        };
        if screen.interrupted() {
            let _ = self.cancel.send(true);
        }
        self.state.advance(Instant::now());
        let state = &self.state;
        let _ = screen.draw(|frame| view::render(state, frame.area(), frame.buffer_mut()));
    }
}

impl Drop for Ui {
    fn drop(&mut self) {
        if self.screen.take().is_some() && !self.diagnostics.is_empty() {
            let color = std::env::var_os("NO_COLOR").is_none();
            let text = Text::from(std::mem::take(&mut self.diagnostics));
            let _ = report::write(&mut io::stderr().lock(), &text, color);
        }
    }
}

fn drain(pending: &mut String, tokenizer: &Tokenizer) -> u64 {
    if pending.is_empty() {
        return 0;
    }
    let count = tokens::count(tokenizer, pending).unwrap_or(0);
    pending.clear();
    count
}

fn trail(record: &RequestRecord) -> Line<'static> {
    let m = &record.metrics;
    let id = format!("#{}", record.request_id).bold();
    let phase = match record.phase {
        Phase::Warmup => "warmup ",
        Phase::Measurement => "",
    }
    .dim();
    let elapsed = seconds(record.elapsed_ms);
    let tokens = m
        .generated_tokens
        .map(|n| format!("{} tok", group(n)))
        .unwrap_or_default();
    if record.status == Status::Completed {
        let mut spans = vec!["  ✓ ".green(), phase, id, format!("  {elapsed}").into()];
        if let Some(ttft) = m.ttft_ms {
            spans.push(format!(" · ttft {}", seconds(ttft)).dim());
        }
        spans.push(format!(" · {tokens}").dim());
        if let Some(r) = m.reasoning_tokens.filter(|r| *r > 0) {
            spans.push(format!(" ({} reasoning)", group(r)).dim());
        }
        if let Some(rate) = m.generation_tokens_per_second {
            spans.push(format!(" · {rate:.1} tok/s").dim());
        }
        if record.stopped_at_output_cap() {
            spans.push(" · output cap".yellow());
        }
        return Line::from(spans);
    }
    let (mark, what, style) = match record.status {
        Status::TimedOut => ("✗", "timed out", Style::new().yellow()),
        Status::Cancelled => ("–", "cancelled", Style::new().yellow()),
        _ => ("✗", "failed", Style::new().red()),
    };
    let detail = match &record.error {
        Some(e) if record.status == Status::Failed => {
            format!(" · {}: {}", e.category, e.message.replace('\n', " "))
        }
        _ if tokens.is_empty() => String::new(),
        _ => format!(" · {tokens}"),
    };
    Line::from(vec![
        Span::styled(format!("  {mark} "), style),
        phase,
        id,
        Span::styled(format!("  {what} after {elapsed}"), style),
        detail.dim(),
    ])
}

impl State {
    fn advance(&mut self, now: Instant) {
        while now.duration_since(self.bucket_at) >= Duration::from_secs(1) {
            self.bucket_at += Duration::from_secs(1);
            self.buckets.push(0);
        }
    }

    fn stage_label(&self) -> String {
        match self.stage {
            Stage::Tokenizer => format!("Loading tokenizer {}", self.tokenizer_name),
            Stage::Prompts => format!("Generating {} prompts", self.total),
            Stage::Run(Phase::Warmup) => format!("Warmup: {} requests", self.total),
            Stage::Run(Phase::Measurement) => format!("Measuring: {} requests", self.total),
        }
    }

    fn cancelling(&self) -> bool {
        *self.cancel.borrow()
    }

    fn in_flight(&self) -> impl Iterator<Item = (&u32, &Request)> {
        self.requests.iter().filter(|(_, r)| r.in_flight())
    }

    fn queued(&self) -> usize {
        self.total
            .saturating_sub(self.tally.started + self.in_flight().count())
    }

    fn eta(&self) -> Option<Duration> {
        if self.latency.is_empty() || self.cancelling() {
            return None;
        }
        let mean = Duration::from_secs_f64(
            self.latency.iter().sum::<f64>() / self.latency.len() as f64 / 1000.0,
        );
        let in_flight = self.in_flight().count();
        let slots = in_flight.max(1) as u32;
        let in_flight = self
            .in_flight()
            .map(|(_, r)| mean.saturating_sub(r.started.elapsed()))
            .max()
            .unwrap_or_default();
        Some(in_flight + mean * self.queued() as u32 / slots)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::benchmark::Metrics;

    fn state(requests: u32) -> State {
        let now = Instant::now();
        State {
            model: String::new(),
            api: String::new(),
            url: String::new(),
            settings: String::new(),
            tokenizer_name: String::new(),
            prints_report: true,
            stage: Stage::Run(Phase::Measurement),
            stage_started: now,
            total: requests as usize,
            cancel: watch::channel(false).1,
            requests: (0..requests).map(|id| (id, Request::new())).collect(),
            tally: Counts::default(),
            latency: Vec::new(),
            buckets: vec![0],
            bucket_at: now,
        }
    }

    /// A measurement halfway through, with every kind of request on screen.
    pub(super) fn busy() -> State {
        let now = Instant::now();
        let ago = |s: f64| now - Duration::from_secs_f64(s);
        let mut state = state(0);
        state.model = "gpt-oss:latest".into();
        state.api = "chat".into();
        state.url = "http://localhost:11434/v1".into();
        state.settings = "concurrency 4 · input 550 · cap 1024".into();
        state.total = 40;
        state.stage_started = ago(300.0);
        state.bucket_at = ago(0.4);
        state.buckets = (0..301).map(|i| 40 + (i * 7 % 23) as u64).collect();
        for id in 0..24u32 {
            let started = ago(300.0 - f64::from(id) * 11.0);
            let mut request = Request::new();
            request.started = started;
            let first = started + Duration::from_secs_f64(0.4 + f64::from(id % 5) * 0.3);
            let switch = first + Duration::from_secs(8);
            request.activity = vec![
                (started, Activity::Waiting),
                (first, Activity::Reasoning),
                (switch, Activity::Content),
            ];
            request.first = Some(first);
            request.reasoning = 300 + u64::from(id) * 3;
            request.content = 500;
            if id < 20 {
                let ended = started + Duration::from_secs(40);
                request.ended = Some(ended);
                request.last = Some(ended);
                request.rate = Some(21.5);
                request.outcome = Some(match id {
                    7 => Outcome::Failed("HTTP 500".into()),
                    12 => Outcome::TimedOut,
                    15 => Outcome::Capped,
                    _ => Outcome::Done,
                });
                state.tally.started += 1;
                state.tally.completed += 1;
                state.latency.push(40_000.0);
            } else {
                request.last = Some(ago(if id == 20 { 14.0 } else { 0.2 }));
            }
            state.requests.insert(id, request);
        }
        state
    }

    fn live_ui() -> Ui {
        Ui {
            screen: None,
            state: state(1),
            cancel: watch::channel(false).0,
            diagnostics: Vec::new(),
        }
    }

    fn record(status: Status, generated_tokens: Option<u64>) -> RequestRecord {
        let now = Instant::now();
        RequestRecord {
            request_id: 0,
            phase: Phase::Measurement,
            start_time_unix_ns: 0,
            end_time_unix_ns: 0,
            elapsed_ms: 0.0,
            status,
            http_status: Some(200),
            finish_reason: None,
            input_target_tokens: 1,
            output_cap: Some(32),
            metrics: Metrics {
                generated_tokens,
                ..Metrics::default()
            },
            reasoning_kinds: Vec::new(),
            provider_usage: None,
            error: None,
            start: now,
            end: now,
        }
    }

    fn delta(content: &str, reasoning: &str) -> Delta {
        Delta {
            id: 0,
            content: content.into(),
            reasoning: reasoning.into(),
            at: Instant::now(),
        }
    }

    #[test]
    fn finished_requests_count_pending_and_queued_text_once() {
        for status in [
            Status::Completed,
            Status::Failed,
            Status::TimedOut,
            Status::Cancelled,
        ] {
            for queued in [false, true] {
                let mut ui = live_ui();
                ui.delta(delta("", "a"));
                if !queued {
                    ui.delta(delta("b c", ""));
                }
                ui.finished(&record(status, Some(3)));
                assert_eq!(ui.state.buckets.iter().sum::<u64>(), 3);
                if queued {
                    ui.delta(delta("b c", ""));
                }
                ui.tokenize(&tokens::test_tokenizer());
                assert_eq!(ui.state.buckets.iter().sum::<u64>(), 3);
            }
        }
    }

    #[test]
    fn finishing_counts_only_the_remainder_in_the_current_bucket() {
        let mut ui = live_ui();
        ui.delta(delta("b", "a"));
        ui.tokenize(&tokens::test_tokenizer());
        assert_eq!(ui.state.buckets.iter().sum::<u64>(), 2);
        ui.delta(delta(" c", ""));
        ui.state.bucket_at = Instant::now() - Duration::from_secs(1);
        ui.finished(&record(Status::Completed, Some(3)));
        assert_eq!(ui.state.buckets, vec![2, 1]);
        ui.tokenize(&tokens::test_tokenizer());
        assert_eq!(ui.state.buckets, vec![2, 1]);
    }

    #[test]
    fn missing_or_smaller_final_counts_do_not_add_tokens() {
        for generated in [None, Some(1)] {
            let mut ui = live_ui();
            for content in ["hel", "lo"] {
                ui.delta(delta(content, ""));
                ui.tokenize(&tokens::test_tokenizer());
            }
            assert_eq!(ui.state.buckets.iter().sum::<u64>(), 2);
            ui.finished(&record(Status::Completed, generated));
            assert_eq!(ui.state.buckets.iter().sum::<u64>(), 2);
        }
    }
}
