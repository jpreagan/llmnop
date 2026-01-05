use anyhow::{Context, Result, anyhow};
use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::types::chat::{
    ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
};
use futures::{Stream, StreamExt};
use serde::Deserialize;
use std::pin::Pin;

#[derive(Debug, Deserialize)]
pub struct StreamDelta {
    pub content: Option<String>,
    pub reasoning_content: Option<String>, // deprecated in vLLM v0.11.1
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
        builder.max_tokens(tokens);
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
