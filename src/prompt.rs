use anyhow::{Result, anyhow, bail};
use rand::RngExt;
use rand_distr::{Distribution, Normal};
use tokenizers::Tokenizer;

pub const CORPUS: &str = include_str!("assets/shakespeare.txt");
pub fn sample_length(
    rng: &mut impl rand::Rng,
    mean: u32,
    stddev: u32,
    minimum: u32,
) -> Result<u32> {
    if stddev == 0 {
        return Ok(mean.max(minimum));
    }
    let normal = Normal::new(f64::from(mean), f64::from(stddev))?;
    for _ in 0..10_000 {
        let sample = normal.sample(rng);
        if sample >= 0.0 && sample.ceil() <= f64::from(u32::MAX) {
            let n = (sample.ceil() as u32).max(1);
            if n >= minimum {
                return Ok(n);
            }
        }
    }
    bail!("could not sample a length within the requested bounds")
}

pub struct PromptGenerator<'a> {
    tokenizer: &'a Tokenizer,
    corpus: Vec<u32>,
}

impl<'a> PromptGenerator<'a> {
    pub fn new(tokenizer: &'a Tokenizer) -> Result<Self> {
        let mut chunks = Vec::new();
        let mut text = String::new();
        let mut chars = 0;
        for line in CORPUS.lines().map(str::trim).filter(|s| !s.is_empty()) {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(line);
            chars += line.chars().count();
            if chars >= 10_000 {
                chunks.push(std::mem::take(&mut text));
                chars = 0;
            }
        }
        if !text.is_empty() {
            chunks.push(text);
        }
        let corpus = tokenizer
            .encode_batch(chunks, false)
            .map_err(|e| anyhow!("corpus tokenization failed: {e}"))?
            .into_iter()
            .flat_map(|e| e.get_ids().to_vec())
            .collect::<Vec<_>>();
        if corpus.is_empty() {
            bail!("tokenizer produced an empty corpus");
        }
        Ok(Self { tokenizer, corpus })
    }

    fn sample(&self, rng: &mut impl rand::Rng, n: usize) -> Vec<u32> {
        let start = rng.random_range(0..self.corpus.len());
        self.corpus
            .iter()
            .cycle()
            .skip(start)
            .take(n)
            .copied()
            .collect()
    }

    pub fn generate(&self, rng: &mut impl rand::Rng, target: u32) -> Result<String> {
        let target = target as usize;
        let mut tokens = self.sample(rng, target);
        for adjustment in 0..=10 {
            let text = self
                .tokenizer
                .decode(&tokens, false)
                .map_err(|e| anyhow!("prompt decode failed: {e}"))?;
            tokens = self
                .tokenizer
                .encode(text.as_str(), false)
                .map_err(|e| anyhow!("prompt tokenization failed: {e}"))?
                .get_ids()
                .to_vec();
            match tokens.len().cmp(&target) {
                std::cmp::Ordering::Equal => return Ok(text),
                _ if adjustment == 10 => break,
                std::cmp::Ordering::Greater => tokens.truncate(target),
                std::cmp::Ordering::Less => tokens.extend(self.sample(rng, target - tokens.len())),
            }
        }
        bail!("could not generate a prompt with {target} tokens after 10 adjustments")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens;
    use rand::{SeedableRng, rngs::StdRng};

    #[test]
    fn corpus_sampling_wraps_more_than_once() {
        let tokenizer = tokens::test_tokenizer();
        let generator = PromptGenerator {
            tokenizer: &tokenizer,
            corpus: vec![1, 2, 3],
        };
        let mut rng = StdRng::seed_from_u64(42);
        let sampled = generator.sample(&mut rng, 10);
        assert_eq!(sampled.len(), 10);
        for pair in sampled.windows(2) {
            assert_eq!(pair[1], pair[0] % 3 + 1);
        }
    }

    #[test]
    fn prompts_reencode_to_target() {
        let tokenizer = tokens::test_tokenizer();
        let generator = PromptGenerator {
            tokenizer: &tokenizer,
            corpus: vec![1, 2, 3, 4],
        };
        for n in [1, 8, 25] {
            let a = generator
                .generate(&mut StdRng::seed_from_u64(42), n)
                .unwrap();
            assert_eq!(tokens::count(&tokenizer, &a).unwrap(), u64::from(n));
        }
    }

    #[test]
    fn verifies_the_last_repair_and_rejects_exhausted_repairs() {
        use tokenizers::decoders::sequence::Sequence;
        use tokenizers::normalizers::replace::{Replace, ReplacePattern};
        for &last in b"kl" {
            let vocab = (b'a'..=last)
                .enumerate()
                .map(|(i, c)| ((c as char).to_string(), i as u32))
                .collect();
            let model = tokenizers::models::wordlevel::WordLevel::builder()
                .vocab(vocab)
                .unk_token("a".into())
                .build()
                .unwrap();
            let mut tokenizer = Tokenizer::new(model);
            tokenizer.with_pre_tokenizer(Some(
                tokenizers::pre_tokenizers::whitespace::WhitespaceSplit,
            ));
            let decoders = (b'a'..last)
                .rev()
                .map(|c| {
                    let next = (c + 1) as char;
                    Replace::new(
                        ReplacePattern::Regex(format!("^{}$", c as char)),
                        format!("{next} {next}"),
                    )
                    .unwrap()
                    .into()
                })
                .collect();
            tokenizer.with_decoder(Some(Sequence::new(decoders)));
            let generator = PromptGenerator {
                tokenizer: &tokenizer,
                corpus: vec![0],
            };
            let result = generator.generate(&mut StdRng::seed_from_u64(42), 1);
            if last == b'k' {
                assert_eq!(result.unwrap(), "k");
            } else {
                assert!(result.is_err());
            }
        }
    }

    #[test]
    fn caps_stay_above_thinking_budget() {
        let mut rng = StdRng::seed_from_u64(2);
        for _ in 0..1000 {
            assert!(sample_length(&mut rng, 1100, 300, 1025).unwrap() > 1024);
        }
    }
}
