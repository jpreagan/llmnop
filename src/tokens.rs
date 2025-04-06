use anyhow::Result;
use once_cell::sync::OnceCell;
use tokenizers::Tokenizer;

fn llama_tokenizer() -> Result<&'static Tokenizer> {
    static INSTANCE: OnceCell<Tokenizer> = OnceCell::new();
    INSTANCE.get_or_try_init(|| {
        let tokenizer = Tokenizer::from_pretrained("hf-internal-testing/llama-tokenizer", None)
            .map_err(|e| anyhow::anyhow!("Failed to load Llama tokenizer: {}", e))?;
        Ok(tokenizer)
    })
}

pub fn initialize_tokenizer(_model_name: &str) -> Result<()> {
    llama_tokenizer().map(|_| ())
}

pub fn count_tokens(text: &str) -> Result<u32> {
    let tokenizer = llama_tokenizer()?;
    let encoding = tokenizer
        .encode(text, false)
        .map_err(|e| anyhow::anyhow!("Tokenization error: {}", e))?;
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

pub fn truncate_to_token_count(text: &str, max_tokens: u32) -> Result<String> {
    if text.is_empty() || max_tokens == 0 {
        return Ok(String::new());
    }

    let total_tokens = count_tokens(text)?;
    if total_tokens <= max_tokens {
        return Ok(text.to_string());
    }

    let max_index =
        find_largest_prefix_index(text, |prefix| Ok(count_tokens(prefix)? <= max_tokens))?;

    Ok(text.chars().take(max_index).collect())
}
