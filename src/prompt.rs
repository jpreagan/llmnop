use crate::tokens;
use anyhow::Result;
use rand::prelude::*;
use rand_distr::Normal;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

static SHAKESPEARE: &str = include_str!("assets/shakespeare.txt");

static TOKENIZED_CORPUS_CACHE: LazyLock<Mutex<HashMap<String, Arc<Vec<u32>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub struct PromptConfig {
    pub mean_input_tokens: u32,
    pub stddev_input_tokens: u32,
}

fn get_tokenized_corpus(tokenizer: &str) -> Result<Arc<Vec<u32>>> {
    let mut cache = TOKENIZED_CORPUS_CACHE.lock().unwrap();

    if let Some(corpus) = cache.get(tokenizer) {
        return Ok(Arc::clone(corpus));
    }

    let text: String = SHAKESPEARE
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    let token_ids = Arc::new(tokens::encode(&text, tokenizer)?);
    cache.insert(tokenizer.to_string(), Arc::clone(&token_ids));

    Ok(token_ids)
}

fn sample_tokens(corpus: &[u32], num_tokens: usize) -> Vec<u32> {
    if corpus.is_empty() || num_tokens == 0 {
        return Vec::new();
    }

    let corpus_size = corpus.len();
    let num_tokens = num_tokens.min(corpus_size);
    let start_idx = rand::rng().random_range(0..corpus_size);
    let end_idx = start_idx + num_tokens;

    if end_idx <= corpus_size {
        corpus[start_idx..end_idx].to_vec()
    } else {
        let mut tokens = corpus[start_idx..].to_vec();
        tokens.extend_from_slice(&corpus[..end_idx - corpus_size]);
        tokens
    }
}

fn sample_num_tokens(mean: u32, stddev: u32) -> u32 {
    if stddev == 0 {
        return mean.max(1);
    }

    let dist = Normal::new(mean as f64, stddev as f64).unwrap();
    let mut rng = rand::rng();

    loop {
        let sample = dist.sample(&mut rng);
        if sample >= 1.0 {
            return sample.ceil() as u32;
        }
    }
}

pub fn generate_prompt(config: &PromptConfig, tokenizer: &str) -> Result<String> {
    let corpus = get_tokenized_corpus(tokenizer)?;

    let num_tokens = sample_num_tokens(config.mean_input_tokens, config.stddev_input_tokens);
    let token_ids = sample_tokens(&corpus, num_tokens as usize);

    tokens::decode(&token_ids, tokenizer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_tokens_returns_requested_count_when_within_corpus() {
        let corpus: Vec<u32> = (0..100).collect();
        let result = sample_tokens(&corpus, 50);
        assert_eq!(result.len(), 50);
    }

    #[test]
    fn sample_tokens_caps_at_corpus_size_when_exceeding() {
        let corpus: Vec<u32> = (0..100).collect();
        let result = sample_tokens(&corpus, 500);
        assert_eq!(result.len(), 100);
    }

    #[test]
    fn sample_tokens_handles_exact_corpus_size() {
        let corpus: Vec<u32> = (0..100).collect();
        let result = sample_tokens(&corpus, 100);
        assert_eq!(result.len(), 100);
    }

    #[test]
    fn sample_tokens_returns_empty_for_empty_corpus() {
        let corpus: Vec<u32> = vec![];
        let result = sample_tokens(&corpus, 50);
        assert!(result.is_empty());
    }

    #[test]
    fn sample_tokens_returns_empty_for_zero_tokens() {
        let corpus: Vec<u32> = (0..100).collect();
        let result = sample_tokens(&corpus, 0);
        assert!(result.is_empty());
    }
}
