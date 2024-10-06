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
}

/// Generates a prompt by sampling random lines from Shakespeare's sonnet.
pub fn generate_prompt(config: &PromptConfig) -> Result<String> {
    let mut rng = thread_rng();

    // Create a Normal distribution for prompt length
    let token_dist = Normal::new(
        config.mean_input_tokens as f64,
        config.stddev_input_tokens as f64,
    )?;

    // Sample a positive integer from the normal distribution for prompt length
    let mut num_prompt_tokens = sample_random_positive_int(&mut rng, &token_dist);

    // Create the base prompt
    let mut prompt = format!(
        "Randomly stream lines from the following text with {} output tokens. Don't generate eos tokens:\n\n",
        config.mean_output_tokens
    );

    // Count the number of tokens in the base prompt
    let base_token_count = count_tokens(&prompt)?;

    // Ensure prompt length is at least as long as the base text
    while num_prompt_tokens < base_token_count {
        num_prompt_tokens = sample_random_positive_int(&mut rng, &token_dist);
    }

    // Calculate remaining tokens available for sampled lines
    let mut remaining_prompt_tokens = num_prompt_tokens - base_token_count;

    // Access shuffled sonnet lines
    let sonnet_lines = get_shuffled_sonnet_lines();

    // Append lines to the prompt until reaching or exceeding the desired token count
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

    Ok(prompt)
}

/// Samples a random positive integer from a normal distribution.
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
        };
        let result = generate_prompt(&config);
        assert!(result.is_ok());
        let prompt = result.unwrap();
        assert!(
            prompt.contains("Randomly stream lines from the following text with 50 output tokens")
        );
        assert!(prompt.len() > 0);
    }

    #[test]
    fn test_generate_prompt_sonnet_lines() {
        let config = PromptConfig {
            mean_input_tokens: 500,
            stddev_input_tokens: 50,
            mean_output_tokens: 200,
        };
        let result = generate_prompt(&config);
        assert!(result.is_ok());
        let prompt = result.unwrap();

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
