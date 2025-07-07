use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokenizers::Tokenizer;

static TOKENIZER_CACHE: Lazy<Mutex<HashMap<String, Arc<Tokenizer>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn get_tokenizer(model_name: &str) -> Result<Arc<Tokenizer>> {
    let mut cache = TOKENIZER_CACHE.lock().unwrap();

    if let Some(tokenizer) = cache.get(model_name) {
        return Ok(tokenizer.clone());
    }

    let tokenizer = Tokenizer::from_pretrained(model_name, None)
        .map_err(|e| anyhow!("Failed to load tokenizer for '{}': {}", model_name, e))?;

    let tokenizer_arc = Arc::new(tokenizer);
    cache.insert(model_name.to_string(), tokenizer_arc.clone());

    Ok(tokenizer_arc)
}

pub fn count_tokens(text: &str, model_name: &str) -> Result<u32> {
    let tokenizer = get_tokenizer(model_name)?;
    let encoding = tokenizer
        .encode(text, false)
        .map_err(|e| anyhow!("Tokenization error for model '{}': {}", model_name, e))?;
    Ok(encoding.get_ids().len() as u32)
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

    if is_within_limit(text)? {
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

pub fn truncate_to_token_count(text: &str, max_tokens: u32, model_name: &str) -> Result<String> {
    if text.is_empty() || max_tokens == 0 {
        return Ok(String::new());
    }

    let total_tokens = count_tokens(text, model_name)?;
    if total_tokens <= max_tokens {
        return Ok(text.to_string());
    }

    let max_index = find_largest_prefix_index(text, |prefix| {
        Ok(count_tokens(prefix, model_name)? <= max_tokens)
    })?;

    Ok(text.chars().take(max_index).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_TOKENIZER: &str = "hf-internal-testing/llama-tokenizer";

    #[test]
    fn test_truncate_to_token_count() {
        assert_eq!(truncate_to_token_count("", 10, TEST_TOKENIZER).unwrap(), "");

        let short_text = "Hello world";
        let short_token_count = count_tokens(short_text, TEST_TOKENIZER).unwrap();
        assert!(short_token_count < 10);
        assert_eq!(
            truncate_to_token_count(short_text, 10, TEST_TOKENIZER).unwrap(),
            short_text
        );

        let long_text =
            "Shall I compare thee to a summer's day? Thou art more lovely and more temperate.";
        let truncated = truncate_to_token_count(long_text, 5, TEST_TOKENIZER).unwrap();
        assert!(count_tokens(&truncated, TEST_TOKENIZER).unwrap() <= 5);
        assert!(long_text.starts_with(&truncated));
    }

    #[test]
    fn test_find_largest_prefix_index() {
        let text = "The quick brown fox jumps over the lazy dog";

        let result = find_largest_prefix_index(text, |_| Ok(true)).unwrap();
        assert_eq!(result, text.chars().count());

        let result = find_largest_prefix_index(text, |_| Ok(false)).unwrap();
        assert_eq!(result, 0);

        let token_limit = 5;
        let result = find_largest_prefix_index(text, |prefix| {
            Ok(count_tokens(prefix, TEST_TOKENIZER).unwrap() <= token_limit)
        })
        .unwrap();

        let found_prefix: String = text.chars().take(result).collect();
        assert!(count_tokens(&found_prefix, TEST_TOKENIZER).unwrap() <= token_limit);

        if result < text.chars().count() {
            let next_prefix: String = text.chars().take(result + 1).collect();
            assert!(count_tokens(&next_prefix, TEST_TOKENIZER).unwrap() > token_limit);
        }
    }
}
