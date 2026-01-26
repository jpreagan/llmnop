use anyhow::{Context, Result, anyhow};
use async_openai::types::chat::{
    ChatCompletionRequestUserMessageArgs, ChatCompletionStreamOptions, CompletionUsage,
    CreateChatCompletionRequestArgs,
};
use async_openai::{Client, config::OpenAIConfig};
use futures::{Stream, StreamExt};
use serde::Deserialize;
use serde_json::Value;
use std::pin::Pin;

#[derive(Debug, Deserialize)]
pub struct StreamDelta {
    pub content: Option<String>,
    pub reasoning_content: Option<String>,
    pub reasoning: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StreamChoice {
    pub delta: StreamDelta,
}

#[derive(Debug, Deserialize)]
pub struct StreamChunk {
    pub choices: Vec<StreamChoice>,
    pub usage: Option<CompletionUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ResponsesStreamEvent {
    #[serde(rename = "response.output_text.delta")]
    OutputTextDelta {
        #[serde(default, alias = "text")]
        delta: Option<String>,
    },
    #[serde(rename = "response.reasoning_text.delta")]
    ReasoningTextDelta {
        #[serde(default, alias = "text")]
        delta: Option<String>,
    },
    #[serde(rename = "response.reasoning.delta")]
    ReasoningDelta {
        #[serde(default, alias = "text")]
        delta: Option<String>,
    },
    #[serde(rename = "response.completed")]
    ResponseCompleted { response: Option<ResponseCompleted> },
    #[serde(rename = "error")]
    Error {
        #[serde(default)]
        error: Value,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
pub struct ResponseCompleted {
    pub usage: Option<ResponsesUsage>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ResponsesUsage {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    #[serde(default)]
    pub output_tokens_details: Option<ResponsesOutputTokensDetails>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ResponsesOutputTokensDetails {
    pub reasoning_tokens: Option<u32>,
}

pub async fn create_chat_completion_stream(
    client: &Client<OpenAIConfig>,
    model: &str,
    prompt: &str,
    max_tokens: Option<u32>,
    include_usage: bool,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, anyhow::Error>> + Send>>> {
    let mut builder = CreateChatCompletionRequestArgs::default();
    builder
        .model(model)
        .stream(true)
        .messages([ChatCompletionRequestUserMessageArgs::default()
            .content(prompt)
            .build()
            .context("Failed to build message")?
            .into()]);

    if let Some(tokens) = max_tokens {
        builder.max_completion_tokens(tokens);
    }

    if include_usage {
        builder.stream_options(ChatCompletionStreamOptions {
            include_usage: Some(true),
            include_obfuscation: None,
        });
    }

    let request = builder.build().context("Failed to build request")?;

    let stream = client
        .chat()
        .create_stream_byot::<_, StreamChunk>(request)
        .await
        .map_err(|err| anyhow!("OpenAI error: {:?}", err))?;

    let mapped_stream =
        stream.map(|chunk_result| chunk_result.map_err(|err| anyhow!("OpenAI error: {:?}", err)));

    Ok(Box::pin(mapped_stream))
}

pub async fn create_responses_stream(
    client: &Client<OpenAIConfig>,
    model: &str,
    prompt: &str,
    max_tokens: Option<u32>,
) -> Result<Pin<Box<dyn Stream<Item = Result<ResponsesStreamEvent, anyhow::Error>> + Send>>> {
    let mut request = serde_json::json!({
        "model": model,
        "input": prompt,
        "stream": true,
    });

    if let Some(tokens) = max_tokens {
        request["max_output_tokens"] = Value::from(tokens);
    }

    let stream = client
        .responses()
        .create_stream_byot::<_, ResponsesStreamEvent>(request)
        .await
        .map_err(|err| anyhow!("OpenAI error: {:?}", err))?;

    let mapped_stream =
        stream.map(|chunk_result| chunk_result.map_err(|err| anyhow!("OpenAI error: {:?}", err)));

    Ok(Box::pin(mapped_stream))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_chunk_usage_deserialize() {
        let chunk: StreamChunk = serde_json::from_str(
            r#"{"choices":[],"usage":{"prompt_tokens":4,"completion_tokens":6,"total_tokens":10}}"#,
        )
        .expect("deserialize chunk");

        let usage = chunk.usage.expect("usage");
        assert_eq!(usage.prompt_tokens, 4);
        assert_eq!(usage.completion_tokens, 6);
        assert_eq!(usage.total_tokens, 10);
    }

    #[test]
    fn test_output_text_delta_deserialize() {
        let event: ResponsesStreamEvent =
            serde_json::from_str(r#"{"type":"response.output_text.delta","delta":"hello"}"#)
                .expect("deserialize event");

        match event {
            ResponsesStreamEvent::OutputTextDelta { delta } => {
                assert_eq!(delta.as_deref(), Some("hello"));
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[test]
    fn test_output_text_delta_text_alias_deserialize() {
        let event: ResponsesStreamEvent =
            serde_json::from_str(r#"{"type":"response.output_text.delta","text":"hi"}"#)
                .expect("deserialize event");

        match event {
            ResponsesStreamEvent::OutputTextDelta { delta } => {
                assert_eq!(delta.as_deref(), Some("hi"));
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[test]
    fn test_reasoning_text_delta_deserialize() {
        let event: ResponsesStreamEvent =
            serde_json::from_str(r#"{"type":"response.reasoning_text.delta","delta":"thinking"}"#)
                .expect("deserialize event");

        match event {
            ResponsesStreamEvent::ReasoningTextDelta { delta } => {
                assert_eq!(delta.as_deref(), Some("thinking"));
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[test]
    fn test_reasoning_delta_deserialize() {
        let event: ResponsesStreamEvent =
            serde_json::from_str(r#"{"type":"response.reasoning.delta","delta":"chain"}"#)
                .expect("deserialize event");

        match event {
            ResponsesStreamEvent::ReasoningDelta { delta } => {
                assert_eq!(delta.as_deref(), Some("chain"));
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[test]
    fn test_error_event_deserialize() {
        let event: ResponsesStreamEvent =
            serde_json::from_str(r#"{"type":"error","error":{"message":"bad"}} "#)
                .expect("deserialize event");

        match event {
            ResponsesStreamEvent::Error { error } => {
                let message = error.get("message").and_then(|value| value.as_str());
                assert_eq!(message, Some("bad"));
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[test]
    fn test_response_completed_deserialize() {
        let event: ResponsesStreamEvent = serde_json::from_str(
            r#"{"type":"response.completed","response":{"usage":{"input_tokens":3,"output_tokens":5,"total_tokens":8,"output_tokens_details":{"reasoning_tokens":2}}}}"#,
        )
        .expect("deserialize event");

        match event {
            ResponsesStreamEvent::ResponseCompleted { response } => {
                let usage = response.and_then(|response| response.usage).expect("usage");
                assert_eq!(usage.input_tokens, Some(3));
                assert_eq!(usage.output_tokens, Some(5));
                assert_eq!(usage.total_tokens, Some(8));
                assert_eq!(
                    usage
                        .output_tokens_details
                        .and_then(|details| details.reasoning_tokens),
                    Some(2)
                );
            }
            _ => panic!("unexpected event variant"),
        }
    }
}
