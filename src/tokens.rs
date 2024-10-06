use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use tokenizers::Tokenizer;

static TOKENIZER: Lazy<Tokenizer> = Lazy::new(|| {
    Tokenizer::from_pretrained("hf-internal-testing/llama-tokenizer", None)
        .expect("Failed to load Llama 2 tokenizer")
});

pub fn count_tokens(text: &str) -> Result<u32> {
    let encoding = TOKENIZER
        .encode(text, false)
        .map_err(|e| anyhow!("Tokenization error: {}", e))?;
    Ok(encoding.get_ids().len() as u32)
}

pub fn truncate_to_token_count(text: &str, max_tokens: u32) -> Result<String> {
    let mut truncated = String::new();

    for ch in text.chars() {
        let potential_truncated = format!("{}{}", truncated, ch);
        if count_tokens(&potential_truncated)? > max_tokens {
            break;
        }
        truncated.push(ch);
    }

    Ok(truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_tokens() {
        assert_eq!(count_tokens("Hello, world!").unwrap(), 4);
        assert_eq!(count_tokens("").unwrap(), 0);
    }

    #[test]
    fn test_truncate_to_token_count() {
        // Basic truncation
        let text = "Hello, world! This is a test.";
        let max_tokens = 5;
        let truncated = truncate_to_token_count(text, max_tokens).unwrap();
        assert_eq!(count_tokens(&truncated).unwrap(), max_tokens);
        assert_eq!(truncated, "Hello, world! Th");

        // No truncation needed
        let short_text = "Short text";
        let truncated = truncate_to_token_count(short_text, 10).unwrap();
        assert_eq!(truncated, short_text);

        // Truncate to zero tokens
        let truncated = truncate_to_token_count(text, 0).unwrap();
        assert_eq!(truncated, "");

        // Unicode handling
        let unicode_text = "Hello, 世界! This is a test.";
        let truncated = truncate_to_token_count(unicode_text, 7).unwrap();
        assert_eq!(count_tokens(&truncated).unwrap(), 7);
        assert_eq!(truncated, "Hello, 世界! Th");
    }

    #[test]
    fn test_edge_cases() {
        // Empty string
        assert_eq!(truncate_to_token_count("", 5).unwrap(), "");

        // Single character
        let truncated = truncate_to_token_count("a", 1).unwrap();
        assert_eq!(truncated, "a");
        assert_eq!(count_tokens(&truncated).unwrap(), 1);

        // Max tokens larger than text tokens
        let text = "Short text";
        let truncated = truncate_to_token_count(text, 100).unwrap();
        assert_eq!(truncated, text);
    }
}
