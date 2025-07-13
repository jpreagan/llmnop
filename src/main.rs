mod args;
mod benchmark;
mod client;
mod output;
mod prompt;
mod sonnet;
mod tokens;

use anyhow::{Context, Result};
use args::Args;
use async_openai::{Client, config::OpenAIConfig};
use benchmark::run_benchmark;
use clap::Parser;
use futures::{StreamExt, stream::FuturesUnordered};
use indicatif::{ProgressBar, ProgressStyle};
use prompt::{PromptConfig, generate_prompt};
use std::env;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time;

use output::{print_summary_to_stdout, write_results_json};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let api_key = env::var("OPENAI_API_KEY").context("OPENAI_API_KEY not set")?;
    let api_base = env::var("OPENAI_API_BASE").context("OPENAI_API_BASE not set")?;

    let openai_config = OpenAIConfig::new()
        .with_api_key(api_key)
        .with_api_base(api_base);
    let client = Arc::new(Client::with_config(openai_config));

    let overall_start = Instant::now();

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

    let spawn_benchmark = async move |client: Arc<Client<OpenAIConfig>>,
                                      model: String,
                                      prompt: String,
                                      max_tokens: u32| {
        run_benchmark(&client, &model, &prompt, max_tokens).await
    };

    while next_request_index < args.max_num_completed_requests
        && in_flight.len() < args.num_concurrent_requests as usize
    {
        let (ref prompt, max_tokens) = prompts_and_max_tokens[next_request_index as usize];
        let model_name = args.model.clone();
        let prompt_clone = prompt.clone();
        let client_clone = client.clone();

        in_flight.push(tokio::spawn(spawn_benchmark(
            client_clone,
            model_name,
            prompt_clone,
            max_tokens,
        )));
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
                    let client_clone = client.clone();

                    in_flight.push(tokio::spawn(spawn_benchmark(client_clone, model_name, prompt_clone, max_tokens)));
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

    let mut successful_results = Vec::new();
    let mut total_output_tokens = 0_u64;
    let num_errors = all_results.iter().filter(|r| r.is_err()).count();

    for result in &all_results {
        if let Ok(br) = result {
            total_output_tokens += br.output_tokens as u64;
            successful_results.push(br.clone());
        }
    }

    print_summary_to_stdout(
        &successful_results,
        num_errors,
        total_output_tokens,
        overall_start,
        overall_end,
    );

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
    Ok(())
}
