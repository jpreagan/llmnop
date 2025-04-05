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
use benchmark::run_benchmark;
use clap::Parser;
use futures::{stream::FuturesUnordered, StreamExt};
use metrics::Metrics;
use output::display_results;
use prompt::{generate_prompt, PromptConfig};
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

    let mut prompts_and_max_tokens = Vec::with_capacity(args.max_num_completed_requests as usize);
    for _ in 0..args.max_num_completed_requests {
        let (prompt, target_output_tokens) = generate_prompt(&prompt_config, &token_utils)?;
        prompts_and_max_tokens.push((prompt, target_output_tokens));
    }

    let mut all_results = Vec::with_capacity(args.max_num_completed_requests as usize);

    let mut in_flight = FuturesUnordered::new();

    let mut next_request_index = 0;

    while next_request_index < args.max_num_completed_requests
        && in_flight.len() < args.num_concurrent_requests as usize
    {
        let (ref prompt, max_tokens) = prompts_and_max_tokens[next_request_index as usize];

        let model_name = args.model.clone();
        let prompt_clone = prompt.clone();
        let token_utils_clone = token_utils.clone();
        in_flight.push(tokio::spawn(async move {
            run_benchmark(&model_name, &prompt_clone, max_tokens, &token_utils_clone).await
        }));

        next_request_index += 1;
    }

    while !in_flight.is_empty() || next_request_index < args.max_num_completed_requests {
        while next_request_index < args.max_num_completed_requests
            && in_flight.len() < args.num_concurrent_requests as usize
        {
            let (ref prompt, max_tokens) = prompts_and_max_tokens[next_request_index as usize];
            let model_name = args.model.clone();
            let prompt_clone = prompt.clone();
            let token_utils_clone = token_utils.clone();
            in_flight.push(tokio::spawn(async move {
                run_benchmark(&model_name, &prompt_clone, max_tokens, &token_utils_clone).await
            }));

            next_request_index += 1;
        }

        if let Some(done) = in_flight.next().await {
            match done {
                Ok(Ok(benchmark_result)) => {
                    let metrics: Metrics = benchmark_result.clone().into();
                    display_results(&metrics);
                    all_results.push(benchmark_result);
                }
                Ok(Err(e)) => {
                    eprintln!("Request failed: {:?}", e);
                }
                Err(tokio_err) => {
                    eprintln!("Tokio Join Error: {:?}", tokio_err);
                }
            }
        }
    }

    println!("\nAll requests completed. Total: {}", all_results.len());
    Ok(())
}
