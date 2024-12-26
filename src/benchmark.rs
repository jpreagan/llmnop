use crate::client::create_chat_completion_stream;
use crate::tokens::count_tokens;
use anyhow::Result;
use futures::StreamExt;
use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Serialize, Debug)]
pub struct BenchmarkResult {
    pub ttft: Duration,
    pub total_latency: Duration,
    pub throughput: f64,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub inter_token_latency: Vec<Duration>,
}

/// Runs a benchmark for the specified model and prompt.
///
/// # Arguments
///
/// * `model` - The name of the model to benchmark
/// * `prompt` - The prompt to use in the benchmark
/// * `max_tokens` - Maximum number of tokens to generate
///
/// # Returns
///
/// * `Result<BenchmarkResult>` - The results of the benchmark, or an error if the benchmark fails.
pub async fn run_benchmark(model: &str, prompt: &str, max_tokens: u32) -> Result<BenchmarkResult> {
    let mut ttft: Option<Duration> = None;
    let mut inter_token_latency = Vec::new();
    let mut tokens_received: u32 = 0;
    let mut generated_text = String::new();
    let start_time = Instant::now();
    let mut stream = create_chat_completion_stream(model, prompt, max_tokens).await?;
    let mut last_token_time = start_time;
    let input_token_count = count_tokens(prompt)?;

    while let Some(result) = stream.next().await {
        let response = result?;
        for choice in response.choices {
            if let Some(content) = choice.delta.content {
                let now = Instant::now();
                tokens_received += count_tokens(&content)?;

                if ttft.is_none() {
                    ttft = Some(now.duration_since(start_time));
                } else {
                    let latency = now.duration_since(last_token_time);
                    inter_token_latency.push(latency);
                }
                last_token_time = now;
                generated_text.push_str(&content);
            }
        }
    }

    let total_request_time = start_time.elapsed();
    let throughput = if total_request_time.as_secs_f64() > 0.0 {
        tokens_received as f64 / total_request_time.as_secs_f64()
    } else {
        0.0
    };

    Ok(BenchmarkResult {
        ttft: ttft.unwrap_or_else(|| Duration::from_secs(0)),
        total_latency: total_request_time,
        throughput,
        input_tokens: input_token_count,
        output_tokens: count_tokens(&generated_text)?,
        inter_token_latency,
    })
}
