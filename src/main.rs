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

    let (prompt, target_output_tokens) = prompt::generate_prompt(&prompt_config, &token_utils)?;

    let benchmark_result =
        benchmark::run_benchmark(&args.model, &prompt, target_output_tokens, &token_utils).await?;

    let metrics = metrics::Metrics::from(benchmark_result);

    output::display_results(&metrics);

    Ok(())
}
