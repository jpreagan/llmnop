use crate::client::create_chat_completion_stream;
use crate::tokens;
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

pub async fn run_benchmark(model: &str, prompt: &str, max_tokens: u32) -> Result<BenchmarkResult> {
    let start_time = Instant::now();
    let mut chunk_arrivals: Vec<(Instant, String)> = Vec::new();
    let mut generated_text = String::new();

    let mut stream = create_chat_completion_stream(model, prompt, max_tokens).await?;
    while let Some(response_result) = stream.next().await {
        let response = response_result?;
        for choice in response.choices {
            if let Some(content) = choice.delta.content {
                if !content.is_empty() {
                    chunk_arrivals.push((Instant::now(), content.clone()));
                    generated_text.push_str(&content);
                }
            }
        }
    }

    let end_time = Instant::now();

    let input_tokens = tokens::count_tokens(prompt)?;
    let output_tokens = tokens::count_tokens(&generated_text)?;
    let total_tokens = input_tokens + output_tokens;

    Ok(process_benchmark_data(
        start_time,
        end_time,
        &chunk_arrivals,
        input_tokens,
        output_tokens,
        total_tokens,
    ))
}

fn process_benchmark_data(
    start_time: Instant,
    end_time: Instant,
    arrivals: &[(Instant, String)],
    input_tokens: u32,
    output_tokens: u32,
    total_tokens: u32,
) -> BenchmarkResult {
    let mut ttft = Duration::ZERO;
    let mut time_to_next_token = Vec::new();
    let mut last_time = start_time;
    let mut first_non_empty_seen = false;

    for (arrive_time, _) in arrivals.iter() {
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

    let sum_time_to_next_token: Duration = time_to_next_token.iter().sum();

    let inter_token_latency_s = if !arrivals.is_empty() {
        sum_time_to_next_token.as_secs_f64() / arrivals.len() as f64
    } else {
        0.0
    };

    let throughput = if total_latency.as_secs_f64() > 0.0 {
        output_tokens as f64 / total_latency.as_secs_f64()
    } else {
        0.0
    };

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
