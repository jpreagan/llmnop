use anyhow::{anyhow, Result};
use tokenizers::Tokenizer;

#[derive(Clone)]
pub struct TokenUtils {
    tokenizer: Tokenizer,
}

pub fn find_largest_prefix_index(
    text: &str,
    is_within_limit: impl Fn(&str) -> Result<bool>,
) -> Result<usize> {
    if text.is_empty() {
        return Ok(0);
    }

    let chars: Vec<char> = text.chars().collect();
    let char_count = chars.len();

    let full_string: String = chars.iter().collect();
    if is_within_limit(&full_string)? {
        return Ok(char_count);
    }

    let mut low = 0;
    let mut high = char_count;

    while low < high {
        let mid = low + (high - low) / 2;
        let prefix: String = chars[..mid].iter().collect();

        if is_within_limit(&prefix)? {
            low = mid + 1;
        } else {
            high = mid;
        }
    }

    Ok(if low > 0 { low - 1 } else { 0 })
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
        if text.is_empty() || max_tokens == 0 {
            return Ok(String::new());
        }

        let total_tokens = self.count_tokens(text)?;
        if total_tokens <= max_tokens {
            return Ok(text.to_string());
        }

        let max_index =
            find_largest_prefix_index(text, |prefix| Ok(self.count_tokens(prefix)? <= max_tokens))?;

        Ok(text.chars().take(max_index).collect())
    }
}
