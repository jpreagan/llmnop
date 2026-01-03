use crate::client::create_chat_completion_stream;
use crate::tokens;
use anyhow::Result;
use async_openai::{Client, config::OpenAIConfig};
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
    client: &Client<OpenAIConfig>,
    model: &str,
    prompt: &str,
    max_tokens: u32,
    tokenizer: &str,
) -> Result<BenchmarkResult> {
    let start_time = Instant::now();
    let mut chunk_arrivals: Vec<(Instant, String)> = Vec::new();
    let mut generated_text = String::new();

    let mut stream = create_chat_completion_stream(client, model, prompt, max_tokens).await?;
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

    let input_tokens = tokens::count_tokens(prompt, tokenizer)?;
    let output_tokens = tokens::count_tokens(&generated_text, tokenizer)?;
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
            first_non_empty_seen = true;
        } else {
            let gap = arrive_time.duration_since(last_time);
            time_to_next_token.push(gap);
        }
        last_time = *arrive_time;
    }

    let total_latency = end_time.duration_since(start_time);

    let sum_time_to_next_token: Duration = time_to_next_token.iter().sum();

    let inter_token_latency_s = if !time_to_next_token.is_empty() {
        sum_time_to_next_token.as_secs_f64() / time_to_next_token.len() as f64
    } else {
        0.0
    };

    let generation_window = if arrivals.len() >= 2 {
        let first = arrivals.first().unwrap().0;
        let last = arrivals.last().unwrap().0;
        last.saturating_duration_since(first)
    } else {
        Duration::ZERO
    };

    let throughput = if generation_window.as_secs_f64() > 0.0 {
        output_tokens as f64 / generation_window.as_secs_f64()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn test_process_benchmark_data_multiple_arrivals() {
        let now = Instant::now();
        let start_time = now;
        let arr1 = now + Duration::from_millis(64);
        let arr2 = now + Duration::from_millis(128);
        let arr3 = now + Duration::from_millis(192);
        let end_time = arr3;

        let arrivals = vec![
            (arr1, "hello".to_string()),
            (arr2, " world".to_string()),
            (arr3, "!".to_string()),
        ];

        let input_tokens = 10;
        let output_tokens = 3;
        let total_tokens = input_tokens + output_tokens;

        let result = process_benchmark_data(
            start_time,
            end_time,
            &arrivals,
            input_tokens,
            output_tokens,
            total_tokens,
        );

        assert_eq!(result.ttft, Duration::from_millis(64));
        assert_eq!(result.total_latency, Duration::from_millis(192));
        // 192ms - 64ms = 128ms => 3 / 0.128 = 23.4375 tok/s
        assert_eq!(result.throughput, 23.4375);
        assert_eq!(result.input_tokens, 10);
        assert_eq!(result.output_tokens, 3);
        assert_eq!(result.total_tokens, 13);
        // Gap 1: 128-64 = 64ms, Gap 2: 192-128 = 64ms -> Average: 64ms = 0.064s
        assert_eq!(result.inter_token_latency_s, 0.064);
    }

    #[test]
    fn test_throughput_generation_window_example() {
        // chunks arrive at T=[1.0s, 1.2s, 1.5s], output_tokens=30
        // throughput = 30 / (1.5 - 1.0) = 60 tok/s
        let start = Instant::now();
        let arrivals = vec![
            (start + Duration::from_millis(1000), "a".to_string()),
            (start + Duration::from_millis(1200), "b".to_string()),
            (start + Duration::from_millis(1500), "c".to_string()),
        ];

        let input_tokens = 10;
        let output_tokens = 30;
        let total_tokens = input_tokens + output_tokens;

        let end_time = start + Duration::from_millis(1500);

        let result = process_benchmark_data(
            start,
            end_time,
            &arrivals,
            input_tokens,
            output_tokens,
            total_tokens,
        );

        assert_eq!(result.throughput, 60.0);
    }

    #[test]
    fn test_ttft_not_included_in_inter_token_latency() {
        let start_time = Instant::now();
        let ttft_delay = Duration::from_millis(1000);
        let inter_token_gap = Duration::from_millis(100);

        // Mock chunk arrivals: first token after 1s, then 2 more tokens with 100ms gaps
        let arrivals = vec![
            (start_time + ttft_delay, "Hello".to_string()),
            (
                start_time + ttft_delay + inter_token_gap,
                " world".to_string(),
            ),
            (
                start_time + ttft_delay + inter_token_gap * 2,
                "!".to_string(),
            ),
        ];

        let end_time = start_time + ttft_delay + inter_token_gap * 2;

        let result = process_benchmark_data(start_time, end_time, &arrivals, 10, 3, 13);

        assert_eq!(result.ttft, Duration::from_millis(1000));

        // Inter-token latency should only include the 2 gaps between tokens (100ms each)
        // Gap 1: 100ms, Gap 2: 100ms -> Average: 100ms = 0.1s
        assert_eq!(result.inter_token_latency_s, 0.1);
    }

    #[test]
    fn test_single_token_response() {
        let start_time = Instant::now();
        let ttft_delay = Duration::from_millis(1000);

        let arrivals = vec![(start_time + ttft_delay, "Hello".to_string())];

        let end_time = start_time + ttft_delay;

        let result = process_benchmark_data(start_time, end_time, &arrivals, 5, 1, 6);

        assert_eq!(result.ttft, Duration::from_millis(1000));

        // No inter-token latency since there's only one token
        assert_eq!(result.inter_token_latency_s, 0.0);

        // Single chunk => generation window duration = 0 => throughput reported as 0.0
        assert_eq!(result.throughput, 0.0);
    }

    #[test]
    fn test_throughput_independent_of_post_generation_tail() {
        let start = Instant::now();
        let arrivals = vec![
            (start + Duration::from_millis(1000), "a".to_string()),
            (start + Duration::from_millis(1500), "b".to_string()),
        ];

        let input_tokens = 10;
        let output_tokens = 30;
        let total_tokens = input_tokens + output_tokens;

        // Simulate a long tail after last token before the stream finishes
        let end_time = start + Duration::from_millis(10_000);

        let result = process_benchmark_data(
            start,
            end_time,
            &arrivals,
            input_tokens,
            output_tokens,
            total_tokens,
        );

        // Generation window: 1.5s - 1.0s = 0.5s => 30 / 0.5 = 60 tok/s
        assert_eq!(result.throughput, 60.0);
        // But total latency should still include the tail.
        assert_eq!(result.total_latency, Duration::from_millis(10_000));
    }

    #[test]
    fn test_empty_response() {
        let start_time = Instant::now();
        let end_time = start_time + Duration::from_millis(100);

        let arrivals = vec![];

        let result = process_benchmark_data(start_time, end_time, &arrivals, 5, 0, 5);

        assert_eq!(result.ttft, Duration::ZERO);

        assert_eq!(result.inter_token_latency_s, 0.0);
        assert_eq!(result.throughput, 0.0);
    }

    #[test]
    fn test_process_benchmark_data_zero_duration() {
        let now = Instant::now();
        let start_time = now;
        let end_time = now;

        let arrivals = vec![];
        let input_tokens = 10;
        let output_tokens = 0;
        let total_tokens = input_tokens + output_tokens;

        let result = process_benchmark_data(
            start_time,
            end_time,
            &arrivals,
            input_tokens,
            output_tokens,
            total_tokens,
        );

        assert_eq!(result.ttft, Duration::ZERO);
        assert_eq!(result.total_latency, Duration::ZERO);
        assert_eq!(result.throughput, 0.0);
        assert_eq!(result.input_tokens, 10);
        assert_eq!(result.output_tokens, 0);
        assert_eq!(result.total_tokens, 10);
        assert_eq!(result.inter_token_latency_s, 0.0);
    }
}
