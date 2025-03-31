use crate::sonnet::get_shuffled_sonnet_lines;
use crate::tokens::TokenUtils;
use anyhow::Result;
use rand::prelude::*;
use rand_distr::Normal;

pub struct PromptConfig {
    pub mean_input_tokens: u32,
    pub stddev_input_tokens: u32,
    pub mean_output_tokens: u32,
    pub stddev_output_tokens: u32,
}

pub fn generate_prompt(config: &PromptConfig, token_utils: &TokenUtils) -> Result<(String, u32)> {
    let mut rng = rand::rng();

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

    let base_token_count = token_utils.count_tokens(&prompt)?;

    while num_prompt_tokens < base_token_count {
        num_prompt_tokens = sample_random_positive_int(&mut rng, &input_token_dist);
    }

    let mut remaining_prompt_tokens = num_prompt_tokens - base_token_count;

    let sonnet_lines = get_shuffled_sonnet_lines();

    let mut sampling_lines = true;
    while sampling_lines {
        for line in &sonnet_lines {
            let line_token_count = token_utils.count_tokens(line)?;

            if remaining_prompt_tokens < line_token_count {
                let truncated_line =
                    token_utils.truncate_to_token_count(line, remaining_prompt_tokens)?;
                prompt.push_str(&truncated_line);
                remaining_prompt_tokens -= token_utils.count_tokens(&truncated_line)?;
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

fn sample_random_positive_int<R: Rng>(rng: &mut R, dist: &Normal<f64>) -> u32 {
    loop {
        let sample = dist.sample(rng);
        if sample > 0.0 {
            return sample.round() as u32;
        }
    }
}
