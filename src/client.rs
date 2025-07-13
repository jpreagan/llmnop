use anyhow::{Context, Result, anyhow};
use async_openai::types::{
    ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    CreateChatCompletionStreamResponse,
};
use async_openai::{Client, config::OpenAIConfig};
use futures::{Stream, StreamExt};

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
        .map_err(|err| anyhow!("OpenAI error: {:?}", err))?;

    let mapped_stream =
        stream.map(|chunk_result| chunk_result.map_err(|err| anyhow!("OpenAI error: {:?}", err)));

    Ok(mapped_stream)
}
