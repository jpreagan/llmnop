use crate::args::ApiType;
use anyhow::{Context, Result};
use reqwest::{Client, Request};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize)]
pub struct Failure {
    pub category: &'static str,
    pub message: String,
}

impl Failure {
    pub fn new(category: &'static str, message: impl ToString) -> Self {
        Self {
            category,
            message: message.to_string(),
        }
    }
}

pub fn http_client() -> Result<Client> {
    Client::builder()
        .retry(reqwest::retry::never())
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("could not create HTTP client")
}

pub fn request_body(
    api: ApiType,
    model: &str,
    prompt: &str,
    cap: Option<u32>,
    thinking: Option<u32>,
    usage: bool,
) -> Value {
    let mut body = json!({"model": model, "stream": true});
    if api == ApiType::Responses {
        body["input"] = json!(prompt);
    } else {
        body["messages"] = json!([{"role": "user", "content": prompt}]);
    }
    if let Some(cap) = cap {
        let field = match api {
            ApiType::Chat => "max_completion_tokens",
            ApiType::Responses => "max_output_tokens",
            ApiType::Messages => "max_tokens",
        };
        body[field] = json!(cap);
    }
    if api == ApiType::Chat && usage {
        body["stream_options"] = json!({"include_usage": true});
    }
    if let Some(budget) = thinking {
        body["thinking"] = json!({"type":"enabled", "budget_tokens":budget});
    }
    body
}

pub fn build_request(
    client: &Client,
    api: ApiType,
    base: &str,
    key: Option<&str>,
    body: &Value,
) -> Result<Request> {
    let suffix = match api {
        ApiType::Chat => "chat/completions",
        ApiType::Responses => "responses",
        ApiType::Messages => "messages",
    };
    let mut request = client
        .post(format!("{}/{suffix}", base.trim_end_matches('/')))
        .header("accept", "text/event-stream")
        .json(body);
    if api == ApiType::Messages {
        request = request.header("anthropic-version", "2023-06-01");
        if let Some(key) = key.filter(|key| !key.is_empty()) {
            request = request.header("x-api-key", key);
        }
    } else if let Some(key) = key.filter(|key| !key.is_empty()) {
        request = request.bearer_auth(key);
    }
    request.build().context("could not construct request")
}

#[derive(Default)]
pub struct Event<'a> {
    pub content: &'a str,
    pub reasoning: &'a str,
    pub reasoning_kind: Option<&'static str>,
    pub usage: Option<&'a Value>,
    pub finish_reason: Option<&'a str>,
    pub done: bool,
    pub failure: Option<Failure>,
}

fn text<'a>(value: &'a Value, pointer: &str) -> &'a str {
    value.pointer(pointer).and_then(Value::as_str).unwrap_or("")
}

pub fn parse_event(api: ApiType, value: &Value) -> Result<Event<'_>, Failure> {
    if !value.is_object() {
        return Err(Failure::new(
            "protocol",
            "stream event is not a JSON object",
        ));
    }
    let mut event = Event::default();
    if let Some(error) = value.get("error").filter(|v| !v.is_null()) {
        event.failure = Some(Failure::new("provider", error));
    }
    match api {
        ApiType::Chat => {
            event.usage = value.get("usage").filter(|v| v.is_object());
            if let Some(choices) = value.get("choices").and_then(Value::as_array) {
                if choices.len() > 1 {
                    return Err(Failure::new(
                        "protocol",
                        "expected a single completion choice",
                    ));
                }
                if let Some(choice) = choices.first() {
                    if choice
                        .get("index")
                        .and_then(Value::as_u64)
                        .is_some_and(|index| index != 0)
                    {
                        return Err(Failure::new(
                            "protocol",
                            "unexpected completion choice index",
                        ));
                    }
                    event.content = text(choice, "/delta/content");
                    if event.content.is_empty() {
                        event.content = text(choice, "/delta/refusal");
                    }
                    event.reasoning = text(choice, "/delta/reasoning_content");
                    if event.reasoning.is_empty() {
                        event.reasoning = text(choice, "/delta/reasoning");
                    }
                    event.reasoning_kind = Some("text");
                    event.finish_reason = choice.get("finish_reason").and_then(Value::as_str);
                }
            }
        }
        ApiType::Responses => {
            let kind = text(value, "/type");
            let delta = value
                .get("delta")
                .or_else(|| value.get("text"))
                .and_then(Value::as_str)
                .unwrap_or("");
            match kind {
                "response.output_text.delta" | "response.refusal.delta" => event.content = delta,
                "response.reasoning_text.delta" | "response.reasoning.delta" => {
                    event.reasoning = delta;
                    event.reasoning_kind = Some("text");
                }
                "response.reasoning_summary_text.delta" => {
                    event.reasoning = delta;
                    event.reasoning_kind = Some("summary");
                }
                "response.completed" | "response.incomplete" | "response.failed" => {
                    event.usage = value.pointer("/response/usage").filter(|v| v.is_object());
                    event.finish_reason = value
                        .pointer("/response/incomplete_details/reason")
                        .and_then(Value::as_str)
                        .or_else(|| value.pointer("/response/status").and_then(Value::as_str));
                    event.done = kind == "response.completed"
                        || (kind == "response.incomplete"
                            && matches!(
                                event.finish_reason,
                                Some("max_output_tokens" | "content_filter")
                            ));
                    if !event.done {
                        event.failure = Some(Failure::new(
                            "provider",
                            value
                                .pointer("/response/error")
                                .filter(|v| !v.is_null())
                                .map(Value::to_string)
                                .unwrap_or_else(|| {
                                    format!(
                                        "{kind}: {}",
                                        event.finish_reason.unwrap_or("unknown reason")
                                    )
                                }),
                        ));
                    }
                    if event.finish_reason.is_none() && event.done {
                        event.finish_reason = Some("completed");
                    }
                }
                "error" => {
                    event.failure = Some(Failure::new("provider", value));
                }
                _ => {}
            }
        }
        ApiType::Messages => match text(value, "/type") {
            "message_start" => {
                event.usage = value.pointer("/message/usage").filter(|v| v.is_object())
            }
            "content_block_delta" => match text(value, "/delta/type") {
                "text_delta" => event.content = text(value, "/delta/text"),
                "thinking_delta" => {
                    event.reasoning = text(value, "/delta/thinking");
                    event.reasoning_kind = Some("text");
                }
                _ => {}
            },
            "message_delta" => {
                event.usage = value.get("usage").filter(|v| v.is_object());
                event.finish_reason = value.pointer("/delta/stop_reason").and_then(Value::as_str);
            }
            "message_stop" => event.done = true,
            "error" => event.failure = Some(Failure::new("provider", value)),
            _ => {}
        },
    }
    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_and_usage_are_mapped_to_each_api() {
        for (api, key) in [
            (ApiType::Chat, "max_completion_tokens"),
            (ApiType::Responses, "max_output_tokens"),
            (ApiType::Messages, "max_tokens"),
        ] {
            let body = request_body(api, "m", "a b", Some(8), None, true);
            assert_eq!(body[key], 8);
            assert_eq!(body["stream"], true);
            assert_eq!(body.get("stream_options").is_some(), api == ApiType::Chat);
            assert!(
                request_body(api, "m", "a", None, None, false)
                    .get(key)
                    .is_none()
            );
        }
    }

    #[test]
    fn summaries_and_full_reasoning_remain_distinguishable() {
        let value = json!({"type":"response.reasoning_summary_text.delta", "delta":"a"});
        let event = parse_event(ApiType::Responses, &value).unwrap();
        assert_eq!(event.reasoning_kind, Some("summary"));
        assert_eq!(event.reasoning, "a");
    }

    #[test]
    fn output_limit_is_completion_but_failed_response_is_not() {
        let value = json!({"type":"response.incomplete", "response":{"incomplete_details":{"reason":"max_output_tokens"}}});
        assert!(parse_event(ApiType::Responses, &value).unwrap().done);
        let value = json!({"type":"response.failed", "response":{"error":{"message":"bad"}}});
        assert!(
            parse_event(ApiType::Responses, &value)
                .unwrap()
                .failure
                .is_some()
        );
    }
}
