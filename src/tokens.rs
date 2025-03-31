use anyhow::{anyhow, Result};
use tokenizers::Tokenizer;

pub struct TokenUtils {
    tokenizer: Tokenizer,
}

impl TokenUtils {
    pub fn new(model_name: &str) -> Result<Self> {
        let tokenizer = Tokenizer::from_pretrained(model_name, None)
            .map_err(|e| anyhow!("Failed to load tokenizer for {}: {}", model_name, e))?;
        Ok(Self { tokenizer })
    }

    pub fn count_tokens(&self, text: &str) -> Result<u32> {
        let encoding = self
            .tokenizer
            .encode(text, false)
            .map_err(|e| anyhow!("Tokenization error: {}", e))?;
        Ok(encoding.get_ids().len() as u32)
    }

    pub fn truncate_to_token_count(&self, text: &str, max_tokens: u32) -> Result<String> {
        let mut truncated = String::new();

        for ch in text.chars() {
            let potential_truncated = format!("{}{}", truncated, ch);
            if self.count_tokens(&potential_truncated)? > max_tokens {
                break;
            }
            truncated.push(ch);
        }

        Ok(truncated)
    }
}
