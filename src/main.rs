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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let prompt_config = PromptConfig {
        mean_input_tokens: args.mean_input_tokens,
        stddev_input_tokens: args.stddev_input_tokens,
        mean_output_tokens: args.mean_output_tokens,
    };

    let prompt = prompt::generate_prompt(&prompt_config)?;

    let benchmark_result = benchmark::run_benchmark(&args.model, &prompt).await?;

    let metrics = metrics::Metrics::from(benchmark_result);

    output::display_results(&metrics);

    Ok(())
}
