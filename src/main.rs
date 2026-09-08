mod args;
mod benchmark;
mod client;
mod output;
mod prompt;
#[cfg(feature = "self-update")]
mod self_update;
#[cfg(test)]
mod tests;
mod tokens;

use anyhow::Result;
#[cfg(feature = "self-update")]
use args::Command;
use args::{ApiType, Args, OutputFormat};

use benchmark::{BenchmarkRequest, BenchmarkResult, run_benchmark};
use clap::Parser;

use futures::{StreamExt, stream::FuturesUnordered};
use indicatif::{ProgressBar, ProgressStyle};
use prompt::PromptGenerator;
use std::io::{self, IsTerminal};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time;

use output::{BenchmarkConfig, print_summary_to_stdout, write_results_json};

fn unix_time_now_ns() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| u64::try_from(d.as_nanos()).ok())
        .unwrap_or_default()
}

async fn run_benchmark_task(
    client: Arc<reqwest::Client>,
    api_type: ApiType,
    request: BenchmarkRequest,
) -> Result<BenchmarkResult> {
    run_benchmark(&client, api_type, request).await
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    #[cfg(feature = "self-update")]
    if let Some(Command::Update) = args.command {
        return self_update::run_update().await;
    }

    let (url, model) = match args.require_benchmark_args() {
        Ok(values) => values,
        Err(err) => err.exit(),
    };
    let model = model.to_string();
    args.validate().map_err(|err| anyhow::anyhow!("{err}"))?;

    let api = args.api;
    let client = Arc::new(client::http_client()?);

    let tokenizer = args.tokenizer.clone().unwrap_or_else(|| model.clone());
    let use_server_token_count = args.use_server_token_count;
    let overall_start = Instant::now();
    let overall_start_unix_ns = unix_time_now_ns();

    let loaded_tokenizer = Arc::new(tokens::load(&tokenizer)?);
    let generator = PromptGenerator::new(&loaded_tokenizer)?;
    let mut prompts = Vec::with_capacity(args.max_num_completed_requests as usize);
    let mut rng = rand::rng();
    for _ in 0..args.max_num_completed_requests {
        let target =
            prompt::sample_length(&mut rng, args.input_tokens, args.input_tokens_stddev, 1)?;
        let cap = args
            .output_cap
            .map(|mean| {
                prompt::sample_length(
                    &mut rng,
                    mean,
                    args.output_cap_stddev,
                    args.thinking_budget.map_or(1, |b| b + 1),
                )
            })
            .transpose()?;
        prompts.push((generator.generate(&mut rng, target)?, cap));
    }

    let mut all_results = Vec::with_capacity(args.max_num_completed_requests as usize);

    let mut in_flight = FuturesUnordered::new();
    let mut next_request_index = 0;

    let output_format = args.effective_output_format();
    let disable_progress =
        matches!(output_format, OutputFormat::None) || !io::stderr().is_terminal();

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

    while next_request_index < args.max_num_completed_requests
        && in_flight.len() < args.num_concurrent_requests as usize
    {
        let (prompt, max_tokens) = &prompts[next_request_index as usize];
        let model_name = model.clone();
        let prompt_clone = prompt.clone();
        let client_clone = client.clone();
        let tokenizer_clone = loaded_tokenizer.clone();

        let request = BenchmarkRequest {
            model: model_name,
            url: url.to_owned(),
            api_key: args.api_key.clone(),
            timeout: timeout_duration,
            prompt: prompt_clone,
            max_tokens: *max_tokens,
            thinking_budget_tokens: args.thinking_budget,
            tokenizer: tokenizer_clone,
            use_server_token_count,
        };

        in_flight.push(tokio::spawn(run_benchmark_task(client_clone, api, request)));
        next_request_index += 1;
    }

    loop {
        tokio::select! {
            _ = &mut timeout_future, if !timeout_occurred => {
                eprintln!(
                    "\nTimeout reached after {} seconds. Collecting completed results...",
                    args.timeout
                );
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
                    let (prompt, max_tokens) = &prompts[next_request_index as usize];
                    let model_name = model.clone();
                    let prompt_clone = prompt.clone();
                    let client_clone = client.clone();
                    let tokenizer_clone = loaded_tokenizer.clone();


                    let request = BenchmarkRequest {
                        model: model_name,
            url: url.to_owned(),
            api_key: args.api_key.clone(),
            timeout: timeout_duration,
                        prompt: prompt_clone,
                        max_tokens: *max_tokens,
                        thinking_budget_tokens: args.thinking_budget,
                        tokenizer: tokenizer_clone,
                        use_server_token_count,
                    };

                    in_flight.push(tokio::spawn(run_benchmark_task(
                        client_clone,
                        api,
                        request,
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
    let overall_end_unix_ns = unix_time_now_ns();
    if timeout_occurred {
        eprintln!(
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

    let config = BenchmarkConfig {
        model: &model,
        tokenizer: &tokenizer,
        mean_input_tokens: args.input_tokens,
        stddev_input_tokens: args.input_tokens_stddev,
        mean_output_tokens: args.output_cap,
        stddev_output_tokens: args.output_cap_stddev,
        num_concurrent_requests: args.num_concurrent_requests,
    };

    let written_results = write_results_json(
        &config,
        &all_results,
        overall_start,
        overall_end,
        overall_start_unix_ns,
        overall_end_unix_ns,
    )?;

    match output_format {
        OutputFormat::Table => {
            print_summary_to_stdout(
                &successful_results,
                num_errors,
                total_output_tokens,
                total_reasoning_tokens,
                overall_start,
                overall_end,
            );
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string(&written_results.summary)?);
        }
        OutputFormat::None => {}
    }
    Ok(())
}
