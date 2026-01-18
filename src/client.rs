use anyhow::{Context, Result, anyhow};
use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::types::chat::{
    ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
};
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
    #[serde(rename = "error")]
    Error {
        #[serde(default)]
        error: Value,
    },
    #[serde(other)]
    Other,
}

pub async fn create_chat_completion_stream(
    client: &Client<OpenAIConfig>,
    model: &str,
    prompt: &str,
    max_tokens: Option<u32>,
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
    fn test_other_event_deserialize() {
        let event: ResponsesStreamEvent =
            serde_json::from_str(r#"{"type":"response.completed"}"#).expect("deserialize event");

        match event {
            ResponsesStreamEvent::Other => {}
            _ => panic!("unexpected event variant"),
        }
    }
}
