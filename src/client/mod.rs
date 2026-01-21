pub mod anthropic;
pub mod openai;

use anthropic::messages::MessagesClient;
use async_openai::Client;
use async_openai::config::OpenAIConfig;

#[derive(Debug)]
pub enum ApiClient {
    OpenAI(Box<Client<OpenAIConfig>>),
    AnthropicMessages(Box<MessagesClient>),
}

impl ApiClient {
    pub fn openai(&self) -> Option<&Client<OpenAIConfig>> {
        match self {
            ApiClient::OpenAI(client) => Some(client.as_ref()),
            ApiClient::AnthropicMessages(_) => None,
        }
    }

    pub fn anthropic_messages(&self) -> Option<&MessagesClient> {
        match self {
            ApiClient::AnthropicMessages(client) => Some(client.as_ref()),
            ApiClient::OpenAI(_) => None,
        }
    }
}
