use anyhow::{Context, Result, anyhow};
use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::types::chat::{
    ChatCompletionRequestUserMessageArgs, ChatCompletionStreamOptions, CompletionUsage,
    CreateChatCompletionRequestArgs,
};
use futures::{Stream, StreamExt};
use serde::Deserialize;
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
}
