mod args;
mod benchmark;
mod client;
mod output;
mod prompt;
mod report;
#[cfg(feature = "self-update")]
mod self_update;
mod style;
#[cfg(test)]
mod tests;
mod tokens;
mod ui;

use anyhow::{Context, Result};
use args::{Args, Command, OutputFormat};
use benchmark::{Phase, PreparedRequest, RequestRecord};
use clap::Parser;
use output::{BenchmarkSummary, ResultsWriter};
use prompt::PromptGenerator;
use std::collections::VecDeque;
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;
use tokenizers::Tokenizer;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;
use ui::{Stage, Ui};

const INTERRUPTED: u8 = 130;

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
            prompt::sample_length(&mut rng, args.input_tokens, args.input_tokens_stddev)?;
        let cap = args
            .output_cap
            .map(|mean| prompt::sample_length(&mut rng, mean, args.output_cap_stddev))
            .transpose()?;
        let prompt = generator.generate(&mut rng, input_target)?;
        let input_tokens = tokens::count(tokenizer, &prompt)?;
        let body = client::request_body(
            args.api,
            args.model.as_deref().unwrap(),
            &prompt,
            cap,
            args.extra_inputs.as_ref(),
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
    ui: &mut Ui,
) -> Result<Vec<RequestRecord>> {
    let phase = requests.front().map(|r| r.phase);
    let mut in_flight = JoinSet::new();
    let mut processing = JoinSet::new();
    let mut records = Vec::with_capacity(requests.len());
    let timeout = Duration::from_secs_f64(args.request_timeout);
    // Streamed text is only forwarded when a dashboard will count it.
    let (progress, mut deltas) = mpsc::unbounded_channel();
    let progress = ui.interactive().then_some(progress);
    let mut tick = ui::frames();
    if let Some(phase) = phase {
        ui.begin(Stage::Run(phase), requests.len());
    }
    loop {
        while !*cancel.borrow() && in_flight.len() < args.concurrency as usize {
            let Some(request) = requests.pop_front() else {
                break;
            };
            ui.started(request.id, request.output_cap);
            in_flight.spawn(benchmark::capture(
                client.clone(),
                args.api,
                request,
                timeout,
                cancel.clone(),
                progress.clone(),
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
                ui.finished(&record);
                records.push(record);
            }
            Some(delta) = deltas.recv(), if progress.is_some() => ui.delta(delta),
            _ = tick.tick() => {
                ui.tokenize(tokenizer);
                ui.draw();
            }
        }
    }
    Ok(records)
}

fn main() -> Result<ExitCode> {
    let runtime = tokio::runtime::Runtime::new()?;
    let code = runtime.block_on(run());
    // An interrupted tokenizer download holds its blocking thread until the
    // transfer ends. Exit without waiting for it.
    runtime.shutdown_background();
    code
}

async fn run() -> Result<ExitCode> {
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
    let (cancel_tx, cancel) = watch::channel(false);
    let signals = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            let _ = cancel_tx.send(true);
        }
    });
    let mut ui = Ui::new(&args, cancel.clone());
    let Some(tokenizer) = ui
        .attend(Stage::Tokenizer, 0, move || tokens::load(&tokenizer_name))
        .await
    else {
        return Ok(ExitCode::from(INTERRUPTED));
    };
    let tokenizer = Arc::new(tokenizer?);
    let client = client::http_client()?;
    let args = Arc::new(args);
    let prepared = {
        let (args, client, tokenizer) = (Arc::clone(&args), client.clone(), Arc::clone(&tokenizer));
        ui.attend(
            Stage::Prompts,
            (args.warmup + args.requests) as usize,
            move || {
                let generator = PromptGenerator::new(&tokenizer)?;
                let warmup = prepare(&args, &client, &tokenizer, &generator, Phase::Warmup)?;
                let measured = prepare(&args, &client, &tokenizer, &generator, Phase::Measurement)?;
                anyhow::Ok((warmup, measured))
            },
        )
        .await
    };
    let Some(prepared) = prepared else {
        return Ok(ExitCode::from(INTERRUPTED));
    };
    let (warmup, measured) = prepared?;
    let mut writer = ResultsWriter::new(args.results_dir.as_deref()).await?;
    let mut records = run_phase(
        &args,
        &client,
        &tokenizer,
        warmup,
        &cancel,
        &mut writer,
        &mut ui,
    )
    .await?;
    records.extend(
        run_phase(
            &args,
            &client,
            &tokenizer,
            measured,
            &cancel,
            &mut writer,
            &mut ui,
        )
        .await?,
    );
    // Clear the dashboard before the report shares its terminal.
    drop(ui);
    let interrupted = *cancel.borrow();
    signals.abort();
    let summary = BenchmarkSummary::new(&args, writer.run_id.clone(), &records, interrupted);
    writer.finish(&summary).await?;
    let mut stdout = io::stdout().lock();
    match args.format {
        OutputFormat::Table => {
            let color = stdout.is_terminal() && std::env::var_os("NO_COLOR").is_none();
            report::write(
                &mut stdout,
                &report::report(&summary, &writer.directory),
                color,
            )?;
        }
        OutputFormat::Json => {
            serde_json::to_writer(&mut stdout, &summary)?;
            writeln!(stdout)?;
            eprintln!("Results: {}", writer.directory.display());
        }
        OutputFormat::None => eprintln!("Results: {}", writer.directory.display()),
    }
    let failed = summary.measurement.unsuccessful() + summary.warmup.unsuccessful() > 0;
    Ok(if interrupted {
        ExitCode::from(INTERRUPTED)
    } else if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
