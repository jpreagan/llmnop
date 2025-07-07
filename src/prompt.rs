use crate::sonnet::get_shuffled_sonnet_lines;
use crate::tokens;
use anyhow::Result;
use rand::prelude::*;
use rand_distr::Normal;

pub struct PromptConfig {
    pub mean_input_tokens: u32,
    pub stddev_input_tokens: u32,
    pub mean_output_tokens: u32,
    pub stddev_output_tokens: u32,
}

pub fn generate_prompt(config: &PromptConfig) -> Result<(String, u32)> {
    let mut rng = rand::rng();

    let input_token_dist = Normal::new(
        config.mean_input_tokens as f64,
        config.stddev_input_tokens as f64,
    )?;

    let output_token_dist = Normal::new(
        config.mean_output_tokens as f64,
        config.stddev_output_tokens as f64,
    )?;

    let target_output_tokens = output_token_dist
        .sample_iter(&mut rng)
        .map(|x| x.round() as u32)
        .find(|&x| x > 0)
        .unwrap();

    let mut num_prompt_tokens = input_token_dist
        .sample_iter(&mut rng)
        .map(|x| x.round() as u32)
        .find(|&x| x > 0)
        .unwrap();

    let mut prompt = format!(
        "Randomly stream lines from the following text with {} output tokens. Don't generate eos tokens:\n\n",
        target_output_tokens
    );

    let base_token_count = tokens::count_tokens(&prompt)?;

    while num_prompt_tokens < base_token_count {
        num_prompt_tokens = input_token_dist
            .sample_iter(&mut rng)
            .map(|x| x.round() as u32)
            .find(|&x| x > 0)
            .unwrap();
    }

    let mut remaining_prompt_tokens = num_prompt_tokens - base_token_count;

    let sonnet_lines = get_shuffled_sonnet_lines();

    let mut sampling_lines = true;
    while sampling_lines {
        for line in &sonnet_lines {
            let line_token_count = tokens::count_tokens(line)?;

            if remaining_prompt_tokens < line_token_count {
                let truncated_line =
                    tokens::truncate_to_token_count(line, remaining_prompt_tokens)?;
                prompt.push_str(&truncated_line);
                remaining_prompt_tokens -= tokens::count_tokens(&truncated_line)?;
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
