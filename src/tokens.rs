use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use tokenizers::Tokenizer;

static TOKENIZER_CACHE: LazyLock<Mutex<HashMap<String, Arc<Tokenizer>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

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

pub fn encode_batch(texts: &[String], model_name: &str) -> Result<Vec<Vec<u32>>> {
    let tokenizer = get_tokenizer(model_name)?;
    let inputs: Vec<&str> = texts.iter().map(|text| text.as_str()).collect();
    let encodings = tokenizer
        .encode_batch(inputs, false)
        .map_err(|e| anyhow!("Tokenization error for model '{}': {}", model_name, e))?;
    Ok(encodings
        .into_iter()
        .map(|encoding| encoding.get_ids().to_vec())
        .collect())
}

pub fn decode(token_ids: &[u32], model_name: &str) -> Result<String> {
    let tokenizer = get_tokenizer(model_name)?;
    let text = tokenizer
        .decode(token_ids, true)
        .map_err(|e| anyhow!("Decoding error for model '{}': {}", model_name, e))?;
    Ok(text)
}
