use anyhow::{anyhow, Context, Result};
use async_openai::types::{
    ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    CreateChatCompletionStreamResponse,
};
use async_openai::{config::OpenAIConfig, error::OpenAIError, Client};
use futures::{Stream, StreamExt};

fn map_openai_error(err: OpenAIError) -> anyhow::Error {
    anyhow!("OpenAI error: {:?}", err)
}

pub async fn create_chat_completion_stream(
    client: &Client<OpenAIConfig>,
    model: &str,
    prompt: &str,
    max_tokens: u32,
) -> Result<impl Stream<Item = Result<CreateChatCompletionStreamResponse, anyhow::Error>>> {
    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .max_tokens(max_tokens)
        .messages([ChatCompletionRequestUserMessageArgs::default()
            .content(prompt)
            .build()
            .context("Failed to build message")?
            .into()])
        .build()
        .context("Failed to build request")?;

    let stream = client
        .chat()
        .create_stream(request)
        .await
        .map_err(map_openai_error)?;

    let mapped_stream = stream.map(|chunk_result| chunk_result.map_err(map_openai_error));

    Ok(mapped_stream)
}
