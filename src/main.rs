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

use anyhow::{Context, Result};
use args::{Args, Command, OutputFormat};
use benchmark::{Phase, PreparedRequest, RequestRecord};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use output::{BenchmarkSummary, ResultsWriter};
use prompt::PromptGenerator;
use std::collections::VecDeque;
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;
use tokenizers::Tokenizer;
use tokio::sync::watch;
use tokio::task::JoinSet;

fn prepare(
    args: &Args,
    client: &reqwest::Client,
    tokenizer: &Tokenizer,
    generator: &PromptGenerator<'_>,
    phase: Phase,
) -> Result<VecDeque<PreparedRequest>> {
    let warmup = phase == Phase::Warmup;
    let count = if warmup { args.warmup } else { args.requests };
    let mut prepared = VecDeque::with_capacity(count as usize);
    let mut rng = rand::rng();
    for index in 0..count {
        let input_target =
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
        let prompt = generator.generate(&mut rng, input_target)?;
        let input_tokens = tokens::count(tokenizer, &prompt)?;
        let body = client::request_body(
            args.api,
            args.model.as_deref().unwrap(),
            &prompt,
            cap,
            args.thinking_budget,
            args.request_usage,
        );
        prepared.push_back(PreparedRequest {
            id: if warmup { index } else { args.warmup + index },
            phase,
            input_target,
            input_tokens,
            output_cap: cap,
            request: client::build_request(
                client,
                args.api,
                args.url.as_deref().unwrap(),
                args.api_key.as_deref(),
                &body,
            )?,
        });
    }
    Ok(prepared)
}

async fn run_phase(
    args: &Args,
    client: &reqwest::Client,
    tokenizer: &Arc<Tokenizer>,
    mut requests: VecDeque<PreparedRequest>,
    cancel: &watch::Receiver<bool>,
    writer: &mut ResultsWriter,
) -> Result<Vec<RequestRecord>> {
    let pb = if args.no_progress || !io::stderr().is_terminal() {
        ProgressBar::hidden()
    } else {
        ProgressBar::new(requests.len() as u64)
    };
    pb.set_style(ProgressStyle::with_template(
        "{spinner} [{elapsed_precise}] {pos}/{len}",
    )?);
    let mut in_flight = JoinSet::new();
    let mut processing = JoinSet::new();
    let mut records = Vec::with_capacity(requests.len());
    let timeout = Duration::from_secs_f64(args.request_timeout);
    loop {
        while !*cancel.borrow() && in_flight.len() < args.concurrency as usize {
            let Some(request) = requests.pop_front() else {
                break;
            };
            in_flight.spawn(benchmark::capture(
                client.clone(),
                args.api,
                request,
                timeout,
                cancel.clone(),
            ));
        }
        if in_flight.is_empty() && processing.is_empty() {
            break;
        }
        tokio::select! {
            Some(captured) = in_flight.join_next(), if !in_flight.is_empty() => {
                let captured = captured.context("request task failed")?;
                let tokenizer = Arc::clone(tokenizer);
                processing.spawn_blocking(move || captured.finish(&tokenizer));
            }
            Some(record) = processing.join_next(), if !processing.is_empty() => {
                let record = record.context("token accounting task failed")?;
                writer.append(&record).await?;
                if let Some(error) = &record.error { if pb.is_hidden() { eprintln!("Request {}: {}", record.request_id, error.message); } else { pb.println(format!("Request {}: {}", record.request_id, error.message)); } }
                records.push(record);
                pb.inc(1);
            }
        }
    }
    pb.finish_and_clear();
    Ok(records)
}

#[tokio::main]
async fn main() -> Result<ExitCode> {
    let mut args = Args::parse();
    if let Some(Command::Update) = args.command {
        #[cfg(feature = "self-update")]
        self_update::run_update().await?;
        #[cfg(not(feature = "self-update"))]
        eprintln!(
            "Self-update is only available for standalone installs. Use your package manager to upgrade."
        );
        return Ok(ExitCode::SUCCESS);
    }
    if let Err(error) = args.validate() {
        error.exit();
    }
    let tokenizer_name = args
        .tokenizer
        .get_or_insert_with(|| args.model.clone().unwrap())
        .clone();
    let tokenizer = Arc::new(tokens::load(&tokenizer_name)?);
    let client = client::http_client()?;
    let generator = PromptGenerator::new(&tokenizer)?;
    let warmup = prepare(&args, &client, &tokenizer, &generator, Phase::Warmup)?;
    let measured = prepare(&args, &client, &tokenizer, &generator, Phase::Measurement)?;
    drop(generator);
    let mut writer = ResultsWriter::new(args.results_dir.as_deref()).await?;
    let (cancel_tx, cancel) = watch::channel(false);
    let signals = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            let _ = cancel_tx.send(true);
        }
    });
    let mut records = run_phase(&args, &client, &tokenizer, warmup, &cancel, &mut writer).await?;
    records.extend(run_phase(&args, &client, &tokenizer, measured, &cancel, &mut writer).await?);
    let interrupted = *cancel.borrow();
    signals.abort();
    let summary = BenchmarkSummary::new(&args, writer.run_id.clone(), &records, interrupted);
    writer.finish(&summary).await?;
    match args.format {
        OutputFormat::Table => io::stdout().lock().write_all(summary.table().as_bytes())?,
        OutputFormat::Json => {
            let mut stdout = io::stdout().lock();
            serde_json::to_writer(&mut stdout, &summary)?;
            writeln!(stdout)?;
        }
        OutputFormat::None => {}
    }
    eprintln!("Results: {}", writer.directory.display());
    let failed = summary.measurement.unsuccessful() + summary.warmup.unsuccessful() > 0;
    Ok(if interrupted {
        ExitCode::from(130)
    } else if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
