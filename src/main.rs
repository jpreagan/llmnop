mod args;
mod benchmark;
mod client;
mod metrics;
mod output;
mod prompt;
mod sonnet;
mod tokens;

use anyhow::Result;
use args::Args;
use clap::Parser;
use metrics::Metrics;
use output::display_results;
use prompt::PromptConfig;
use tokens::TokenUtils;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let token_utils = TokenUtils::new(&args.model)?;

    let prompt_config = PromptConfig {
        mean_input_tokens: args.mean_input_tokens,
        stddev_input_tokens: args.stddev_input_tokens,
        mean_output_tokens: args.mean_output_tokens,
        stddev_output_tokens: args.stddev_output_tokens,
    };

    let mut all_results = Vec::with_capacity(args.max_num_completed_requests as usize);

    for i in 0..args.max_num_completed_requests {
        let (prompt, target_output_tokens) = prompt::generate_prompt(&prompt_config, &token_utils)?;

        println!("--- Request #{} ---", i + 1);
        println!(
            "Approx Input Token Count: {}",
            token_utils.count_tokens(&prompt)?
        );
        println!("Max Tokens to Generate  : {}", target_output_tokens);

        let benchmark_result =
            benchmark::run_benchmark(&args.model, &prompt, target_output_tokens, &token_utils)
                .await?;

        let metrics: Metrics = benchmark_result.as_metrics();

        display_results(&metrics);

        all_results.push(benchmark_result);
    }

    Ok(())
}
