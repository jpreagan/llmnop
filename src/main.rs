mod args;
mod benchmark;
mod client;
mod metrics;
mod output;
mod prompt;
mod sonnet;
mod tokens;

use anyhow::{anyhow, Result};
use args::Args;
use benchmark::run_benchmark;
use clap::Parser;
use futures::{stream::FuturesUnordered, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use prompt::{generate_prompt, PromptConfig};
use std::env;
use std::time::{Duration, Instant};
use tokio::time;

use output::write_results_json;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if env::var("OPENAI_API_KEY").is_err() {
        return Err(anyhow!("OPENAI_API_KEY environment variable not set."));
    }

    if env::var("OPENAI_API_BASE").is_err() {
        return Err(anyhow!("OPENAI_API_BASE environment variable not set."));
    }

    let overall_start = Instant::now();

    println!("Using model: {}", &args.model);
    println!("Tokenizers will be downloaded from Hugging Face Hub and cached on first use.");

    let prompt_config = PromptConfig {
        mean_input_tokens: args.mean_input_tokens,
        stddev_input_tokens: args.stddev_input_tokens,
        mean_output_tokens: args.mean_output_tokens,
        stddev_output_tokens: args.stddev_output_tokens,
    };

    let mut prompts_and_max_tokens = Vec::with_capacity(args.max_num_completed_requests as usize);
    for _ in 0..args.max_num_completed_requests {
        let (prompt, target_output_tokens) = generate_prompt(&prompt_config, &args.model)?;
        prompts_and_max_tokens.push((prompt, target_output_tokens));
    }

    let mut all_results = Vec::with_capacity(args.max_num_completed_requests as usize);

    let mut in_flight = FuturesUnordered::new();
    let mut next_request_index = 0;

    let pb = ProgressBar::new(args.max_num_completed_requests as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("##-"),
    );
    pb.tick();

    let timeout_duration = Duration::from_secs(args.timeout);
    let timeout_future = time::sleep(timeout_duration);
    let mut timeout_occurred = false;

    tokio::pin!(timeout_future);

    while next_request_index < args.max_num_completed_requests
        && in_flight.len() < args.num_concurrent_requests as usize
    {
        let (ref prompt, max_tokens) = prompts_and_max_tokens[next_request_index as usize];
        let model_name = args.model.clone();
        let prompt_clone = prompt.clone();

        in_flight.push(tokio::spawn(async move {
            run_benchmark(&model_name, &prompt_clone, max_tokens).await
        }));
        next_request_index += 1;
    }

    loop {
        tokio::select! {
            _ = &mut timeout_future, if !timeout_occurred => {
                println!("\nTimeout reached after {} seconds. Collecting completed results...", args.timeout);
                timeout_occurred = true;
            }

            Some(done) = in_flight.next(), if !in_flight.is_empty() => {
                match done {
                    Ok(Ok(benchmark_result)) => {
                        all_results.push(Ok(benchmark_result));
                    }
                    Ok(Err(e)) => {
                        eprintln!("Request failed: {:?}", e);
                        all_results.push(Err(e.to_string()));
                    }
                    Err(tokio_err) => {
                        eprintln!("Tokio Join Error: {:?}", tokio_err);
                        all_results.push(Err(format!("Tokio Join Error: {:?}", tokio_err)));
                    }
                }

                pb.inc(1);

                if !timeout_occurred && next_request_index < args.max_num_completed_requests {
                    let (ref prompt, max_tokens) = prompts_and_max_tokens[next_request_index as usize];
                    let model_name = args.model.clone();
                    let prompt_clone = prompt.clone();

                    in_flight.push(tokio::spawn(async move {
                        run_benchmark(&model_name, &prompt_clone, max_tokens).await
                    }));
                    next_request_index += 1;
                }
            }

            _ = async {}, if in_flight.is_empty() => {
                break;
            }
        }

        if all_results.len() >= args.max_num_completed_requests as usize {
            break;
        }
    }

    pb.finish_and_clear();

    let overall_end = Instant::now();
    if timeout_occurred {
        println!(
            "Benchmark terminated due to timeout after {} seconds.",
            args.timeout
        );
    }

    write_results_json(
        &args.results_dir,
        &args.model,
        args.mean_input_tokens,
        args.stddev_input_tokens,
        args.mean_output_tokens,
        args.stddev_output_tokens,
        args.num_concurrent_requests,
        &all_results,
        overall_start,
        overall_end,
    )?;

    println!("Benchmark complete!");
    Ok(())
}
