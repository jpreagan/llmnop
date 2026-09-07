use anyhow::{Result, anyhow};
use std::path::Path;
use tokenizers::Tokenizer;

pub fn load(name: &str) -> Result<Tokenizer> {
    let mut tokenizer = if Path::new(name).is_file() {
        Tokenizer::from_file(name)
    } else {
        Tokenizer::from_pretrained(name, None)
    }
    .map_err(|e| anyhow!("failed to load tokenizer {name}: {e}"))?;
    tokenizer
        .with_truncation(None)
        .map_err(|e| anyhow!("{e}"))?;
    tokenizer.with_padding(None);
    Ok(tokenizer)
}

pub fn count(tokenizer: &Tokenizer, text: &str) -> Result<u64> {
    Ok(tokenizer
        .encode(text, false)
        .map_err(|e| anyhow!("tokenization failed: {e}"))?
        .len() as u64)
}

#[cfg(test)]
pub fn test_tokenizer() -> Tokenizer {
    let vocab = [
        ("[UNK]".into(), 0),
        ("a".into(), 1),
        ("b".into(), 2),
        ("c".into(), 3),
        ("d".into(), 4),
    ]
    .into_iter()
    .collect();
    let model = tokenizers::models::wordlevel::WordLevel::builder()
        .vocab(vocab)
        .unk_token("[UNK]".into())
        .build()
        .unwrap();
    let mut tokenizer = Tokenizer::new(model);
    tokenizer.with_pre_tokenizer(Some(
        tokenizers::pre_tokenizers::whitespace::WhitespaceSplit,
    ));
    tokenizer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_tokenizer_loading_disables_padding_and_truncation() {
        let mut tokenizer = test_tokenizer();
        tokenizer
            .with_truncation(Some(tokenizers::TruncationParams {
                max_length: 1,
                ..Default::default()
            }))
            .unwrap();
        tokenizer.with_padding(Some(tokenizers::PaddingParams {
            strategy: tokenizers::PaddingStrategy::Fixed(4),
            ..Default::default()
        }));
        let path = std::env::temp_dir().join(format!(
            "llmnop-tokenizer-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        tokenizer.save(&path, false).unwrap();
        let loaded = load(path.to_str().unwrap()).unwrap();
        assert_eq!(count(&loaded, "a b c").unwrap(), 3);
        assert!(loaded.get_padding().is_none());
        assert!(loaded.get_truncation().is_none());
        std::fs::remove_file(path).unwrap();
    }
}
