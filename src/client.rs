use anyhow::{anyhow, Result};
use async_openai::types::{
    ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    CreateChatCompletionStreamResponse,
};
use async_openai::{config::OpenAIConfig, error::OpenAIError, Client};
use futures::Stream;
use std::env;

/// Creates a chat completion stream using the OpenAI API.
///
/// # Arguments
///
/// * `model` - The name of the model to use
/// * `prompt` - The prompt to send to the model
/// * `max_tokens` - Maximum number of tokens to generate
///
/// # Returns
///
/// * `Result<impl Stream<Item = Result<CreateChatCompletionStreamResponse, OpenAIError>>>` -
///   A chat completion stream response, or an error if the API request fails.
pub async fn create_chat_completion_stream(
    model: &str,
    prompt: &str,
    max_tokens: u32,
) -> Result<impl Stream<Item = Result<CreateChatCompletionStreamResponse, OpenAIError>>> {
    let api_key = env::var("OPENAI_API_KEY").map_err(|_| anyhow!("OPENAI_API_KEY not set"))?;
    let api_base = env::var("OPENAI_API_BASE").map_err(|_| anyhow!("OPENAI_API_BASE not set"))?;

    let config = OpenAIConfig::new()
        .with_api_key(api_key)
        .with_api_base(api_base);
    let client = Client::with_config(config);

    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .max_tokens(max_tokens)
        .messages([ChatCompletionRequestUserMessageArgs::default()
            .content(prompt)
            .build()?
            .into()])
        .build()?;

    let stream = client.chat().create_stream(request).await?;

    Ok(stream)
}
