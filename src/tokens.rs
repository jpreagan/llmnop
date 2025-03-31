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
