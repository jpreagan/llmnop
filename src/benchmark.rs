use crate::client::create_chat_completion_stream;
use crate::tokens::TokenUtils;
use anyhow::Result;
use futures::StreamExt;
use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkResult {
    pub ttft: Duration,
    pub total_latency: Duration,
    pub throughput: f64,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub inter_token_latency_s: f64,
    pub total_tokens: u32,
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

    let end_time = Instant::now();
    let input_tokens = token_utils.count_tokens(prompt)?;

    Ok(process_chunk_arrivals(
        start_time,
        end_time,
        &chunk_arrivals,
        input_tokens,
    ))
}

fn process_chunk_arrivals(
    start_time: Instant,
    end_time: Instant,
    arrivals: &[(Instant, String)],
    input_tokens: u32,
) -> BenchmarkResult {
    let mut ttft = Duration::ZERO;
    let mut output_tokens = 0_u32;
    let mut time_to_next_token = Vec::new();
    let mut last_time = start_time;
    let mut first_non_empty_seen = false;

    for (arrive_time, chunk_text) in arrivals.iter() {
        if chunk_text.is_empty() {
            continue;
        }
        output_tokens += 1;

        if !first_non_empty_seen {
            ttft = arrive_time.duration_since(start_time);
            time_to_next_token.push(ttft);
            first_non_empty_seen = true;
        } else {
            let gap = arrive_time.duration_since(last_time);
            time_to_next_token.push(gap);
        }
        last_time = *arrive_time;
    }

    let total_latency = end_time.duration_since(start_time);
    let sum_time_to_next_token = time_to_next_token
        .iter()
        .fold(Duration::ZERO, |acc, &x| acc + x);

    let inter_token_latency_s = if output_tokens > 0 {
        sum_time_to_next_token.as_secs_f64() / output_tokens as f64
    } else {
        0.0
    };

    let throughput = if total_latency.as_secs_f64() > 0.0 {
        output_tokens as f64 / total_latency.as_secs_f64()
    } else {
        0.0
    };

    let total_tokens = input_tokens + output_tokens;

    BenchmarkResult {
        ttft,
        total_latency,
        throughput,
        input_tokens,
        output_tokens,
        inter_token_latency_s,
        total_tokens,
    }
}
