use crate::client::create_chat_completion_stream;
use anyhow::Result;
use futures::StreamExt;
use std::io::{stdout, Write};
use std::time::Duration;

pub struct BenchmarkResult {
    pub ttft: Duration,
    pub total_latency: Duration,
    pub throughput: f64,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Runs a benchmark for the specified model and prompt.
///
/// # Arguments
///
/// * `model` - The name of the model to benchmark
/// * `prompt` - The prompt to use in the benchmark
///
/// # Returns
///
/// * `Result<BenchmarkResult>` - The results of the benchmark, or an error if the benchmark fails.
pub async fn run_benchmark(model: &str, prompt: &str) -> Result<BenchmarkResult> {
    let mut stream = create_chat_completion_stream(model, prompt).await?;

    let mut lock = stdout().lock();
    while let Some(result) = stream.next().await {
        match result {
            Ok(response) => {
                response.choices.iter().for_each(|chat_choice| {
                    if let Some(ref content) = chat_choice.delta.content {
                        write!(lock, "{}", content).unwrap();
                    }
                });
            }
            Err(err) => {
                writeln!(lock, "error: {err}").unwrap();
            }
        }
        stdout().flush()?;
    }

    Ok(BenchmarkResult {
        ttft: Duration::ZERO,
        total_latency: Duration::ZERO,
        throughput: 0.0,
        input_tokens: 0,
        output_tokens: 0,
    })
}
