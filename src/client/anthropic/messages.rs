use anyhow::{Context, Result, anyhow};
use futures::{Stream, StreamExt};
use reqwest::Client as HttpClient;
use serde::Deserialize;
use std::pin::Pin;
use std::str;

const ANTHROPIC_MESSAGES_VERSION: &str = "2023-06-01";

#[derive(Debug, Clone)]
pub struct MessagesClient {
    http: HttpClient,
    base_url: String,
    api_key: String,
    version: String,
}

impl MessagesClient {
    pub fn new(base_url: String, api_key: String) -> Self {
        Self {
            http: HttpClient::new(),
            base_url,
            api_key,
            version: ANTHROPIC_MESSAGES_VERSION.to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum MessagesStreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: MessagesMessageStart },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { delta: MessagesContentDelta },
    #[serde(rename = "message_delta")]
    MessageDelta {
        #[serde(default)]
        usage: Option<MessagesUsage>,
    },
    #[serde(rename = "error")]
    Error { error: MessagesError },
    #[serde(rename = "ping")]
    Ping,
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum MessagesContentDelta {
    #[serde(rename = "text_delta")]
    TextDelta {
        #[serde(default)]
        text: Option<String>,
    },
    #[serde(rename = "thinking_delta")]
    ThinkingDelta {
        #[serde(default)]
        thinking: Option<String>,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MessagesUsage {
    #[serde(default)]
    pub input_tokens: Option<u32>,
    #[serde(default)]
    pub output_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct MessagesMessageStart {
    #[serde(default)]
    pub usage: Option<MessagesUsage>,
}

#[derive(Debug, Deserialize)]
pub struct MessagesError {
    #[serde(default)]
    pub message: Option<String>,
}

pub async fn create_messages_stream(
    client: &MessagesClient,
    model: &str,
    prompt: &str,
    max_tokens: u32,
) -> Result<Pin<Box<dyn Stream<Item = Result<MessagesStreamEvent, anyhow::Error>> + Send>>> {
    let url = build_messages_url(&client.base_url)?;
    let request = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": [
            {
                "role": "user",
                "content": prompt
            }
        ],
        "stream": true
    });

    let response = client
        .http
        .post(url)
        .header("x-api-key", &client.api_key)
        .header("anthropic-version", &client.version)
        .header("accept", "text/event-stream")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|err| anyhow!("Anthropic Messages error: {}", err))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let detail = body.trim();
        let detail = if detail.is_empty() {
            "empty response body"
        } else {
            detail
        };
        return Err(anyhow!(
            "Anthropic Messages error (status {}): {}",
            status,
            detail
        ));
    }

    let stream = response.bytes_stream();
    Ok(Box::pin(messages_event_stream(stream)))
}

fn build_messages_url(base_url: &str) -> Result<String> {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(anyhow!("Anthropic Messages base URL is empty"));
    }
    if trimmed.ends_with("/v1/messages") {
        Ok(trimmed.to_string())
    } else if trimmed.ends_with("/v1") {
        Ok(format!("{trimmed}/messages"))
    } else {
        Ok(format!("{trimmed}/v1/messages"))
    }
}

fn messages_event_stream<S, C>(
    stream: S,
) -> impl Stream<Item = Result<MessagesStreamEvent, anyhow::Error>> + Send
where
    S: Stream<Item = Result<C, reqwest::Error>> + Send + Unpin + 'static,
    C: AsRef<[u8]> + Send + 'static,
{
    struct State<S> {
        stream: S,
        buffer: Vec<u8>,
        done: bool,
    }

    futures::stream::try_unfold(
        State {
            stream,
            buffer: Vec::new(),
            done: false,
        },
        |mut state| async move {
            loop {
                if let Some((idx, delim_len)) = find_event_boundary(&state.buffer) {
                    let event_bytes: Vec<u8> = state.buffer.drain(..idx).collect();
                    state.buffer.drain(..delim_len);
                    if let Some(data) = parse_sse_data(&event_bytes)? {
                        let trimmed = data.trim();
                        if trimmed.is_empty() || trimmed == "[DONE]" {
                            continue;
                        }
                        let event = serde_json::from_str::<MessagesStreamEvent>(trimmed).map_err(
                            |err| anyhow!("Anthropic Messages event parse error: {}", err),
                        )?;
                        return Ok(Some((event, state)));
                    }
                    continue;
                }

                if state.done {
                    return Ok(None);
                }

                match state.stream.next().await {
                    Some(Ok(chunk)) => state.buffer.extend_from_slice(chunk.as_ref()),
                    Some(Err(err)) => {
                        return Err(anyhow!("Anthropic Messages stream error: {}", err));
                    }
                    None => state.done = true,
                }
            }
        },
    )
}

fn parse_sse_data(event_bytes: &[u8]) -> Result<Option<String>> {
    if event_bytes.is_empty() {
        return Ok(None);
    }
    let text =
        str::from_utf8(event_bytes).context("Anthropic Messages stream contained invalid UTF-8")?;
    let mut data_lines = Vec::new();
    for line in text.lines() {
        if line.starts_with(':') || line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.trim_start().to_string());
        }
    }
    if data_lines.is_empty() {
        Ok(None)
    } else {
        Ok(Some(data_lines.join("\n")))
    }
}

fn find_event_boundary(buffer: &[u8]) -> Option<(usize, usize)> {
    let mut boundary: Option<(usize, usize)> = None;
    if let Some(pos) = buffer.windows(2).position(|window| window == b"\n\n") {
        boundary = Some((pos, 2));
    }
    if let Some(pos) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
        let replace = match boundary {
            Some((current, _)) => pos < current,
            None => true,
        };
        if replace {
            boundary = Some((pos, 4));
        }
    }
    boundary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_messages_content_block_text_delta_deserialize() {
        let event: MessagesStreamEvent = serde_json::from_str(
            r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"hello"}}"#,
        )
        .expect("deserialize event");

        match event {
            MessagesStreamEvent::ContentBlockDelta { delta } => match delta {
                MessagesContentDelta::TextDelta { text } => {
                    assert_eq!(text.as_deref(), Some("hello"));
                }
                _ => panic!("unexpected delta variant"),
            },
            _ => panic!("unexpected event variant"),
        }
    }

    #[test]
    fn test_messages_content_block_thinking_delta_deserialize() {
        let event: MessagesStreamEvent = serde_json::from_str(
            r#"{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"hmm"}}"#,
        )
        .expect("deserialize event");

        match event {
            MessagesStreamEvent::ContentBlockDelta { delta } => match delta {
                MessagesContentDelta::ThinkingDelta { thinking } => {
                    assert_eq!(thinking.as_deref(), Some("hmm"));
                }
                _ => panic!("unexpected delta variant"),
            },
            _ => panic!("unexpected event variant"),
        }
    }

    #[test]
    fn test_messages_message_delta_usage_deserialize() {
        let event: MessagesStreamEvent = serde_json::from_str(
            r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"input_tokens":3,"output_tokens":5}}"#,
        )
        .expect("deserialize event");

        match event {
            MessagesStreamEvent::MessageDelta { usage, .. } => {
                let usage = usage.expect("usage");
                assert_eq!(usage.input_tokens, Some(3));
                assert_eq!(usage.output_tokens, Some(5));
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[test]
    fn test_messages_message_start_usage_deserialize() {
        let event: MessagesStreamEvent = serde_json::from_str(
            r#"{"type":"message_start","message":{"usage":{"input_tokens":7,"output_tokens":0}}}"#,
        )
        .expect("deserialize event");

        match event {
            MessagesStreamEvent::MessageStart { message } => {
                let usage = message.usage.expect("usage");
                assert_eq!(usage.input_tokens, Some(7));
                assert_eq!(usage.output_tokens, Some(0));
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[test]
    fn test_messages_error_event_deserialize() {
        let event: MessagesStreamEvent =
            serde_json::from_str(r#"{"type":"error","error":{"message":"bad"}}"#)
                .expect("deserialize event");

        match event {
            MessagesStreamEvent::Error { error } => {
                assert_eq!(error.message.as_deref(), Some("bad"));
            }
            _ => panic!("unexpected event variant"),
        }
    }
}
