use crate::sonnet::get_shuffled_sonnet_lines;
use crate::tokens::{count_tokens, truncate_to_token_count};
use anyhow::Result;
use rand::distributions::Distribution;
use rand::thread_rng;
use rand_distr::Normal;

pub struct PromptConfig {
    pub mean_input_tokens: u32,
    pub stddev_input_tokens: u32,
    pub mean_output_tokens: u32,
    pub stddev_output_tokens: u32,
}

/// Generates a prompt by sampling random lines from Shakespeare's sonnet.
///
/// # Arguments
///
/// * `config` - Configuration parameters for prompt generation.
///
/// # Returns
///
/// * `Result<(String, u32)>` - A tuple containing the generated prompt and target output tokens
pub fn generate_prompt(config: &PromptConfig) -> Result<(String, u32)> {
    let mut rng = thread_rng();

    let input_token_dist = Normal::new(
        config.mean_input_tokens as f64,
        config.stddev_input_tokens as f64,
    )?;

    let output_token_dist = Normal::new(
        config.mean_output_tokens as f64,
        config.stddev_output_tokens as f64,
    )?;

    let target_output_tokens = sample_random_positive_int(&mut rng, &output_token_dist);
    let mut num_prompt_tokens = sample_random_positive_int(&mut rng, &input_token_dist);

    let mut prompt = format!(
        "Randomly stream lines from the following text with {} output tokens. Don't generate eos tokens:\n\n",
        target_output_tokens
    );

    let base_token_count = count_tokens(&prompt)?;

    while num_prompt_tokens < base_token_count {
        num_prompt_tokens = sample_random_positive_int(&mut rng, &input_token_dist);
    }

    let mut remaining_prompt_tokens = num_prompt_tokens - base_token_count;

    let sonnet_lines = get_shuffled_sonnet_lines();

    let mut sampling_lines = true;
    while sampling_lines {
        for line in &sonnet_lines {
            let line_token_count = count_tokens(line)?;

            if remaining_prompt_tokens < line_token_count {
                let truncated_line = truncate_to_token_count(line, remaining_prompt_tokens)?;
                prompt.push_str(&truncated_line);
                remaining_prompt_tokens -= count_tokens(&truncated_line)?;
                sampling_lines = false;
                break;
            } else {
                prompt.push_str(line);
                prompt.push('\n');
                remaining_prompt_tokens -= line_token_count;
            }
        }
    }

    Ok((prompt, target_output_tokens))
}

/// Samples a random positive integer from a normal distribution.
///
/// # Arguments
///
/// * `rng` - A mutable reference to a random number generator
/// * `dist` - A reference to a Normal distribution
///
/// # Returns
///
/// * `u32` - A random positive integer sampled from the given distribution.
fn sample_random_positive_int<R: rand::Rng>(rng: &mut R, dist: &Normal<f64>) -> u32 {
    loop {
        let sample = dist.sample(rng);
        if sample > 0.0 {
            return sample.round() as u32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_prompt_basic() {
        let config = PromptConfig {
            mean_input_tokens: 100,
            stddev_input_tokens: 10,
            mean_output_tokens: 50,
            stddev_output_tokens: 5,
        };
        let result = generate_prompt(&config);
        assert!(result.is_ok());
        let (prompt, target_tokens) = result.unwrap();
        assert!(prompt.contains(&format!("with {} output tokens", target_tokens)));
        assert!(prompt.len() > 0);
    }

    #[test]
    fn test_generate_prompt_sonnet_lines() {
        let config = PromptConfig {
            mean_input_tokens: 500,
            stddev_input_tokens: 50,
            mean_output_tokens: 200,
            stddev_output_tokens: 20,
        };
        let result = generate_prompt(&config);
        assert!(result.is_ok());
        let (prompt, _) = result.unwrap();

        let sonnet_lines = get_shuffled_sonnet_lines();
        let mut contains_sonnet_line = false;
        for line in sonnet_lines {
            if prompt.contains(&line) {
                contains_sonnet_line = true;
                break;
            }
        }
        assert!(contains_sonnet_line);
    }
}
