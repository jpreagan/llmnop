use crate::client::create_chat_completion_stream;
use crate::tokens::TokenUtils;
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

pub async fn run_benchmark(
    model: &str,
    prompt: &str,
    max_tokens: u32,
    token_utils: &TokenUtils,
) -> Result<BenchmarkResult> {
    let start_time = Instant::now();
    let mut chunk_arrivals: Vec<(Instant, String)> = Vec::new();

    let mut stream = create_chat_completion_stream(model, prompt, max_tokens).await?;
    while let Some(response_result) = stream.next().await {
        let response = response_result?;
        for choice in response.choices {
            if let Some(content) = choice.delta.content {
                chunk_arrivals.push((Instant::now(), content));
            }
        }
    }

    process_chunk_arrivals(start_time, &chunk_arrivals, prompt, token_utils)
}

pub fn process_chunk_arrivals(
    start_time: Instant,
    arrivals: &[(Instant, String)],
    prompt: &str,
    token_utils: &TokenUtils,
) -> Result<BenchmarkResult> {
    let input_tokens = token_utils.count_tokens(prompt)?;

    let mut ttft = Duration::ZERO;
    let mut output_tokens = 0_u32;
    let mut inter_token_latency = Vec::new();

    if arrivals.is_empty() {
        return Ok(BenchmarkResult {
            ttft,
            total_latency: Duration::ZERO,
            throughput: 0.0,
            input_tokens,
            output_tokens,
            inter_token_latency,
        });
    }

    let mut last_time = start_time;
    let mut first_non_empty_seen = false;
    for (i, (arrive_time, chunk_text)) in arrivals.iter().enumerate() {
        if !chunk_text.is_empty() {
            output_tokens += 1;
            if !first_non_empty_seen {
                ttft = arrive_time.duration_since(start_time);
                first_non_empty_seen = true;
            }
        }

        if i > 0 {
            let gap = arrive_time.duration_since(last_time);
            inter_token_latency.push(gap);
        }
        last_time = *arrive_time;
    }

    let total_latency = last_time.duration_since(start_time);

    let throughput = if total_latency.as_secs_f64() > 0.0 {
        output_tokens as f64 / total_latency.as_secs_f64()
    } else {
        0.0
    };

    Ok(BenchmarkResult {
        ttft,
        total_latency,
        throughput,
        input_tokens,
        output_tokens,
        inter_token_latency,
    })
}
