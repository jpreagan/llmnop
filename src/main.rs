mod args;
mod prompt;
mod sonnet;
mod tokens;

use anyhow::Result;
use args::Args;
use clap::Parser;
use prompt::PromptConfig;
use serde_json;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let config = serde_json::json!({
        "model": args.model,
        "base_url": args.base_url,
        "iterations": args.num_iterations,
        "concurrency": args.concurrency,
        "mean_input_tokens": args.mean_input_tokens,
        "stddev_input_tokens": args.stddev_input_tokens,
        "mean_output_tokens": args.mean_output_tokens,
        "stddev_output_tokens": args.stddev_output_tokens,
    });

    println!("Starting LLMNOP benchmark with the following configuration:");
    println!("{}", serde_json::to_string_pretty(&config)?);

    let prompt_config = PromptConfig {
        mean_input_tokens: args.mean_input_tokens,
        stddev_input_tokens: args.stddev_input_tokens,
        mean_output_tokens: args.mean_output_tokens,
    };

    let generated_prompt = prompt::generate_prompt(&prompt_config)?;
    let token_count = tokens::count_tokens(&generated_prompt)?;

    println!("\nGenerated prompt:");
    println!("{}", generated_prompt);
    println!("Token count: {}", token_count);

    Ok(())
}
