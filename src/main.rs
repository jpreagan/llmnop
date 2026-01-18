mod args;
mod benchmark;
mod client;
mod output;
mod prompt;
mod tokens;

use anyhow::Result;
use args::Args;
use async_openai::{Client, config::OpenAIConfig};
use benchmark::run_benchmark;
use clap::Parser;
use futures::{StreamExt, stream::FuturesUnordered};
use indicatif::{ProgressBar, ProgressStyle};
use prompt::{PromptConfig, generate_prompt};
use rand::prelude::*;
use rand_distr::Normal;
use std::io::{self, IsTerminal};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time;

use output::{BenchmarkConfig, print_summary_to_stdout, write_results_json};

fn sample_max_tokens(mean: u32, stddev: u32) -> u32 {
    if stddev == 0 {
        return mean.max(1);
    }

    let dist = Normal::new(mean as f64, stddev as f64).unwrap();
    let mut rng = rand::rng();

    loop {
        let sample = dist.sample(&mut rng);
        if sample >= 1.0 {
            return sample.ceil() as u32;
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let openai_config = OpenAIConfig::new()
        .with_api_key(args.api_key.clone())
        .with_api_base(args.url.clone());
    let client = Arc::new(Client::with_config(openai_config));

    let tokenizer = args.tokenizer.clone().unwrap_or_else(|| args.model.clone());
    let api = args.api;

    let overall_start = Instant::now();

    let prompt_config = PromptConfig {
        mean_input_tokens: args.mean_input_tokens,
        stddev_input_tokens: args.stddev_input_tokens,
    };

    let mut prompts = Vec::with_capacity(args.max_num_completed_requests as usize);
    for _ in 0..args.max_num_completed_requests {
        let prompt = generate_prompt(&prompt_config, &tokenizer)?;
        prompts.push(prompt);
    }

    let mut all_results = Vec::with_capacity(args.max_num_completed_requests as usize);

    let mut in_flight = FuturesUnordered::new();
    let mut next_request_index = 0;

    let disable_progress = args.no_progress || !io::stderr().is_terminal();

    let pb = if disable_progress {
        ProgressBar::hidden()
    } else {
        let pb = ProgressBar::new(args.max_num_completed_requests as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} ({eta})",
                )
                .unwrap()
                .progress_chars("##-"),
        );
        pb.tick();
        pb
    };

    let timeout_duration = Duration::from_secs(args.timeout);
    let timeout_future = time::sleep(timeout_duration);
    let mut timeout_occurred = false;

    tokio::pin!(timeout_future);

    let spawn_benchmark = async move |client: Arc<Client<OpenAIConfig>>,
                                      api_type,
                                      model: String,
                                      prompt: String,
                                      max_tokens: Option<u32>,
                                      tokenizer: String| {
        run_benchmark(&client, api_type, &model, &prompt, max_tokens, &tokenizer).await
    };

    while next_request_index < args.max_num_completed_requests
        && in_flight.len() < args.num_concurrent_requests as usize
    {
        let prompt = &prompts[next_request_index as usize];
        let model_name = args.model.clone();
        let prompt_clone = prompt.clone();
        let client_clone = client.clone();
        let tokenizer_clone = tokenizer.clone();
        let max_tokens = args
            .mean_output_tokens
            .map(|mean| sample_max_tokens(mean, args.stddev_output_tokens));

        in_flight.push(tokio::spawn(spawn_benchmark(
            client_clone,
            api,
            model_name,
            prompt_clone,
            max_tokens,
            tokenizer_clone,
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
                    let prompt = &prompts[next_request_index as usize];
                    let model_name = args.model.clone();
                    let prompt_clone = prompt.clone();
                    let client_clone = client.clone();
                    let tokenizer_clone = tokenizer.clone();
                    let max_tokens = args
                        .mean_output_tokens
                        .map(|mean| sample_max_tokens(mean, args.stddev_output_tokens));

                    in_flight.push(tokio::spawn(spawn_benchmark(
                        client_clone,
                        api,
                        model_name,
                        prompt_clone,
                        max_tokens,
                        tokenizer_clone,
                    )));
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
    let mut total_reasoning_tokens = 0_u64;
    let num_errors = all_results.iter().filter(|r| r.is_err()).count();

    for br in all_results.iter().flatten() {
        total_output_tokens += br.output_tokens as u64;
        total_reasoning_tokens += br.reasoning_tokens as u64;
        successful_results.push(br.clone());
    }

    print_summary_to_stdout(
        &successful_results,
        num_errors,
        total_output_tokens,
        total_reasoning_tokens,
        overall_start,
        overall_end,
    );

    let config = BenchmarkConfig {
        model: &args.model,
        tokenizer: &tokenizer,
        mean_input_tokens: args.mean_input_tokens,
        stddev_input_tokens: args.stddev_input_tokens,
        mean_output_tokens: args.mean_output_tokens,
        stddev_output_tokens: args.stddev_output_tokens,
        num_concurrent_requests: args.num_concurrent_requests,
    };

    write_results_json(
        &args.results_dir,
        &config,
        &all_results,
        overall_start,
        overall_end,
    )?;
    Ok(())
}
