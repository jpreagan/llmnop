use crate::args::ApiType;
use crate::client::{ResponsesStreamEvent, create_chat_completion_stream, create_responses_stream};
use crate::tokens;
use anyhow::Result;
use async_openai::{Client, config::OpenAIConfig};
use futures::StreamExt;
use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkResult {
    pub ttft: Duration,
    pub ttfo: Option<Duration>,
    pub total_latency: Duration,
    pub throughput: f64,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub reasoning_tokens: u32,
    pub inter_token_latency_s: f64,
    pub total_tokens: u32,
}

struct TokenCounts {
    input: u32,
    output: u32,
    reasoning: u32,
    total: u32,
}

pub async fn run_benchmark(
    client: &Client<OpenAIConfig>,
    api: ApiType,
    model: &str,
    prompt: &str,
    max_tokens: Option<u32>,
    tokenizer: &str,
) -> Result<BenchmarkResult> {
    match api {
        ApiType::Chat => run_chat_benchmark(client, model, prompt, max_tokens, tokenizer).await,
        ApiType::Responses => {
            run_responses_benchmark(client, model, prompt, max_tokens, tokenizer).await
        }
    }
}

async fn run_chat_benchmark(
    client: &Client<OpenAIConfig>,
    model: &str,
    prompt: &str,
    max_tokens: Option<u32>,
    tokenizer: &str,
) -> Result<BenchmarkResult> {
    let start_time = Instant::now();
    let mut content_arrivals: Vec<(Instant, String)> = Vec::new();
    let mut reasoning_arrivals: Vec<(Instant, String)> = Vec::new();
    let mut generated_text = String::new();
    let mut reasoning_text = String::new();

    let mut stream = create_chat_completion_stream(client, model, prompt, max_tokens).await?;
    while let Some(response_result) = stream.next().await {
        let response = response_result?;
        for choice in response.choices {
            let now = Instant::now();

            let reasoning = choice
                .delta
                .reasoning_content
                .as_deref()
                .or(choice.delta.reasoning.as_deref())
                .unwrap_or("");

            if !reasoning.is_empty() {
                reasoning_arrivals.push((now, reasoning.to_string()));
                reasoning_text.push_str(reasoning);
            }

            let content = choice.delta.content.as_deref().unwrap_or("");
            if !content.is_empty() {
                content_arrivals.push((now, content.to_string()));
                generated_text.push_str(content);
            }
        }
    }

    let end_time = Instant::now();

    let token_counts = compute_token_counts(prompt, &generated_text, &reasoning_text, tokenizer)?;

    Ok(process_benchmark_data(
        start_time,
        end_time,
        &content_arrivals,
        &reasoning_arrivals,
        &token_counts,
    ))
}

async fn run_responses_benchmark(
    client: &Client<OpenAIConfig>,
    model: &str,
    prompt: &str,
    max_tokens: Option<u32>,
    tokenizer: &str,
) -> Result<BenchmarkResult> {
    let start_time = Instant::now();
    let mut content_arrivals: Vec<(Instant, String)> = Vec::new();
    let mut reasoning_arrivals: Vec<(Instant, String)> = Vec::new();
    let mut generated_text = String::new();
    let mut reasoning_text = String::new();

    let mut stream = create_responses_stream(client, model, prompt, max_tokens).await?;
    while let Some(event_result) = stream.next().await {
        let event = event_result?;
        let now = Instant::now();

        match event {
            ResponsesStreamEvent::OutputTextDelta { delta: Some(text) } => {
                if !text.is_empty() {
                    content_arrivals.push((now, text.clone()));
                    generated_text.push_str(&text);
                }
            }
            ResponsesStreamEvent::ReasoningTextDelta { delta: Some(text) }
            | ResponsesStreamEvent::ReasoningDelta { delta: Some(text) } => {
                if !text.is_empty() {
                    reasoning_arrivals.push((now, text.clone()));
                    reasoning_text.push_str(&text);
                }
            }
            ResponsesStreamEvent::Error { error } => {
                let message = error
                    .get("message")
                    .and_then(|value| value.as_str())
                    .unwrap_or("unknown Responses API error");
                return Err(anyhow::anyhow!("Responses API error: {}", message));
            }
            _ => {}
        }
    }

    let end_time = Instant::now();

    let token_counts = compute_token_counts(prompt, &generated_text, &reasoning_text, tokenizer)?;

    Ok(process_benchmark_data(
        start_time,
        end_time,
        &content_arrivals,
        &reasoning_arrivals,
        &token_counts,
    ))
}

fn compute_token_counts(
    prompt: &str,
    generated_text: &str,
    reasoning_text: &str,
    tokenizer: &str,
) -> Result<TokenCounts> {
    let input_tokens = tokens::count_tokens(prompt, tokenizer)?;
    let output_tokens = tokens::count_tokens(generated_text, tokenizer)?;
    let reasoning_tokens = if reasoning_text.is_empty() {
        0
    } else {
        tokens::count_tokens(reasoning_text, tokenizer)?
    };

    Ok(TokenCounts {
        input: input_tokens,
        output: output_tokens,
        reasoning: reasoning_tokens,
        total: input_tokens + output_tokens + reasoning_tokens,
    })
}

fn process_benchmark_data(
    start_time: Instant,
    end_time: Instant,
    content_arrivals: &[(Instant, String)],
    reasoning_arrivals: &[(Instant, String)],
    tokens: &TokenCounts,
) -> BenchmarkResult {
    let first_content_time = content_arrivals.first().map(|(t, _)| *t);
    let first_reasoning_time = reasoning_arrivals.first().map(|(t, _)| *t);

    let ttft = match (first_content_time, first_reasoning_time) {
        (Some(c), Some(r)) => std::cmp::min(c, r).duration_since(start_time),
        (Some(c), None) => c.duration_since(start_time),
        (None, Some(r)) => r.duration_since(start_time),
        (None, None) => Duration::ZERO,
    };

    let ttfo = first_content_time.map(|t| t.duration_since(start_time));

    let mut all_arrivals: Vec<Instant> = content_arrivals
        .iter()
        .map(|(t, _)| *t)
        .chain(reasoning_arrivals.iter().map(|(t, _)| *t))
        .collect();
    all_arrivals.sort();

    let mut time_to_next_token = Vec::new();
    let mut last_time: Option<Instant> = None;

    for arrive_time in all_arrivals.iter() {
        if let Some(lt) = last_time {
            let gap = arrive_time.duration_since(lt);
            time_to_next_token.push(gap);
        }
        last_time = Some(*arrive_time);
    }

    let total_latency = end_time.duration_since(start_time);

    let sum_time_to_next_token: Duration = time_to_next_token.iter().sum();

    let inter_token_latency_s = if !time_to_next_token.is_empty() {
        sum_time_to_next_token.as_secs_f64() / time_to_next_token.len() as f64
    } else {
        0.0
    };

    let generation_window = if all_arrivals.len() >= 2 {
        let first = all_arrivals.first().unwrap();
        let last = all_arrivals.last().unwrap();
        last.saturating_duration_since(*first)
    } else {
        Duration::ZERO
    };

    let total_generated_tokens = tokens.output + tokens.reasoning;
    let throughput = if generation_window.as_secs_f64() > 0.0 {
        total_generated_tokens as f64 / generation_window.as_secs_f64()
    } else {
        0.0
    };

    BenchmarkResult {
        ttft,
        ttfo,
        total_latency,
        throughput,
        input_tokens: tokens.input,
        output_tokens: tokens.output,
        reasoning_tokens: tokens.reasoning,
        inter_token_latency_s,
        total_tokens: tokens.total,
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

        let content_arrivals = vec![
            (arr1, "hello".to_string()),
            (arr2, " world".to_string()),
            (arr3, "!".to_string()),
        ];

        let tokens = TokenCounts {
            input: 10,
            output: 3,
            reasoning: 0,
            total: 13,
        };

        let result = process_benchmark_data(start_time, end_time, &content_arrivals, &[], &tokens);

        assert_eq!(result.ttft, Duration::from_millis(64));
        assert_eq!(result.ttfo, Some(Duration::from_millis(64)));
        assert_eq!(result.total_latency, Duration::from_millis(192));
        // 192ms - 64ms = 128ms => 3 / 0.128 = 23.4375 tok/s
        assert_eq!(result.throughput, 23.4375);
        assert_eq!(result.input_tokens, 10);
        assert_eq!(result.output_tokens, 3);
        assert_eq!(result.reasoning_tokens, 0);
        assert_eq!(result.total_tokens, 13);
        // Gap 1: 128-64 = 64ms, Gap 2: 192-128 = 64ms -> Average: 64ms = 0.064s
        assert_eq!(result.inter_token_latency_s, 0.064);
    }

    #[test]
    fn test_throughput_generation_window_example() {
        // chunks arrive at T=[1.0s, 1.2s, 1.5s], output_tokens=30
        // throughput = 30 / (1.5 - 1.0) = 60 tok/s
        let start = Instant::now();
        let content_arrivals = vec![
            (start + Duration::from_millis(1000), "a".to_string()),
            (start + Duration::from_millis(1200), "b".to_string()),
            (start + Duration::from_millis(1500), "c".to_string()),
        ];

        let tokens = TokenCounts {
            input: 10,
            output: 30,
            reasoning: 0,
            total: 40,
        };

        let end_time = start + Duration::from_millis(1500);

        let result = process_benchmark_data(start, end_time, &content_arrivals, &[], &tokens);

        assert_eq!(result.throughput, 60.0);
    }

    #[test]
    fn test_ttft_not_included_in_inter_token_latency() {
        let start_time = Instant::now();
        let ttft_delay = Duration::from_millis(1000);
        let inter_token_gap = Duration::from_millis(100);

        // Mock chunk arrivals: first token after 1s, then 2 more tokens with 100ms gaps
        let content_arrivals = vec![
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

        let tokens = TokenCounts {
            input: 10,
            output: 3,
            reasoning: 0,
            total: 13,
        };

        let result = process_benchmark_data(start_time, end_time, &content_arrivals, &[], &tokens);

        assert_eq!(result.ttft, Duration::from_millis(1000));
        assert_eq!(result.ttfo, Some(Duration::from_millis(1000)));

        // Inter-token latency should only include the 2 gaps between tokens (100ms each)
        // Gap 1: 100ms, Gap 2: 100ms -> Average: 100ms = 0.1s
        assert_eq!(result.inter_token_latency_s, 0.1);
    }

    #[test]
    fn test_single_token_response() {
        let start_time = Instant::now();
        let ttft_delay = Duration::from_millis(1000);

        let content_arrivals = vec![(start_time + ttft_delay, "Hello".to_string())];

        let end_time = start_time + ttft_delay;

        let tokens = TokenCounts {
            input: 5,
            output: 1,
            reasoning: 0,
            total: 6,
        };

        let result = process_benchmark_data(start_time, end_time, &content_arrivals, &[], &tokens);

        assert_eq!(result.ttft, Duration::from_millis(1000));
        assert_eq!(result.ttfo, Some(Duration::from_millis(1000)));

        // No inter-token latency since there's only one token
        assert_eq!(result.inter_token_latency_s, 0.0);

        // Single chunk => generation window duration = 0 => throughput reported as 0.0
        assert_eq!(result.throughput, 0.0);
    }

    #[test]
    fn test_throughput_independent_of_post_generation_tail() {
        let start = Instant::now();
        let content_arrivals = vec![
            (start + Duration::from_millis(1000), "a".to_string()),
            (start + Duration::from_millis(1500), "b".to_string()),
        ];

        let tokens = TokenCounts {
            input: 10,
            output: 30,
            reasoning: 0,
            total: 40,
        };

        // Simulate a long tail after last token before the stream finishes
        let end_time = start + Duration::from_millis(10_000);

        let result = process_benchmark_data(start, end_time, &content_arrivals, &[], &tokens);

        // Generation window: 1.5s - 1.0s = 0.5s => 30 / 0.5 = 60 tok/s
        assert_eq!(result.throughput, 60.0);
        // But total latency should still include the tail.
        assert_eq!(result.total_latency, Duration::from_millis(10_000));
    }

    #[test]
    fn test_empty_response() {
        let start_time = Instant::now();
        let end_time = start_time + Duration::from_millis(100);

        let tokens = TokenCounts {
            input: 5,
            output: 0,
            reasoning: 0,
            total: 5,
        };

        let result = process_benchmark_data(start_time, end_time, &[], &[], &tokens);

        assert_eq!(result.ttft, Duration::ZERO);
        assert_eq!(result.ttfo, None);

        assert_eq!(result.inter_token_latency_s, 0.0);
        assert_eq!(result.throughput, 0.0);
    }

    #[test]
    fn test_process_benchmark_data_zero_duration() {
        let now = Instant::now();
        let start_time = now;
        let end_time = now;

        let tokens = TokenCounts {
            input: 10,
            output: 0,
            reasoning: 0,
            total: 10,
        };

        let result = process_benchmark_data(start_time, end_time, &[], &[], &tokens);

        assert_eq!(result.ttft, Duration::ZERO);
        assert_eq!(result.ttfo, None);
        assert_eq!(result.total_latency, Duration::ZERO);
        assert_eq!(result.throughput, 0.0);
        assert_eq!(result.input_tokens, 10);
        assert_eq!(result.output_tokens, 0);
        assert_eq!(result.reasoning_tokens, 0);
        assert_eq!(result.total_tokens, 10);
        assert_eq!(result.inter_token_latency_s, 0.0);
    }

    #[test]
    fn test_reasoning_tokens_with_content() {
        let start_time = Instant::now();
        let reasoning_start = Duration::from_millis(100);
        let content_start = Duration::from_millis(500);

        // Reasoning tokens arrive first
        let reasoning_arrivals = vec![
            (start_time + reasoning_start, "Let me think...".to_string()),
            (
                start_time + Duration::from_millis(200),
                "Step 1".to_string(),
            ),
            (
                start_time + Duration::from_millis(300),
                "Step 2".to_string(),
            ),
        ];

        // Content tokens arrive after reasoning
        let content_arrivals = vec![
            (start_time + content_start, "The answer is".to_string()),
            (start_time + Duration::from_millis(600), " 42".to_string()),
        ];

        let end_time = start_time + Duration::from_millis(600);

        let tokens = TokenCounts {
            input: 10,
            output: 5,
            reasoning: 10,
            total: 25,
        };

        let result = process_benchmark_data(
            start_time,
            end_time,
            &content_arrivals,
            &reasoning_arrivals,
            &tokens,
        );

        // TTFT should be time to first reasoning token (100ms)
        assert_eq!(result.ttft, Duration::from_millis(100));
        // TTFO should be time to first content token (500ms)
        assert_eq!(result.ttfo, Some(Duration::from_millis(500)));
        assert_eq!(result.output_tokens, 5);
        assert_eq!(result.reasoning_tokens, 10);
        assert_eq!(result.total_tokens, 25);
    }

    #[test]
    fn test_reasoning_only_no_content() {
        let start_time = Instant::now();
        let reasoning_start = Duration::from_millis(100);

        // Only reasoning tokens, no content
        let reasoning_arrivals = vec![
            (start_time + reasoning_start, "Thinking...".to_string()),
            (start_time + Duration::from_millis(200), "Done".to_string()),
        ];

        let end_time = start_time + Duration::from_millis(200);

        let tokens = TokenCounts {
            input: 10,
            output: 0,
            reasoning: 5,
            total: 15,
        };

        let result =
            process_benchmark_data(start_time, end_time, &[], &reasoning_arrivals, &tokens);

        // TTFT should be time to first reasoning token
        assert_eq!(result.ttft, Duration::from_millis(100));
        // TTFO should be None (no content tokens)
        assert_eq!(result.ttfo, None);
        assert_eq!(result.output_tokens, 0);
        assert_eq!(result.reasoning_tokens, 5);
        // Throughput should be based on reasoning tokens
        // Generation window: 200ms - 100ms = 100ms => 5 / 0.1 = 50 tok/s
        assert_eq!(result.throughput, 50.0);
    }

    #[test]
    fn test_content_arrives_before_reasoning() {
        // Edge case: content arrives before reasoning (unusual but possible)
        let start_time = Instant::now();

        let content_arrivals = vec![(start_time + Duration::from_millis(100), "Quick".to_string())];

        let reasoning_arrivals = vec![(
            start_time + Duration::from_millis(200),
            "Wait...".to_string(),
        )];

        let end_time = start_time + Duration::from_millis(200);

        let tokens = TokenCounts {
            input: 10,
            output: 2,
            reasoning: 3,
            total: 15,
        };

        let result = process_benchmark_data(
            start_time,
            end_time,
            &content_arrivals,
            &reasoning_arrivals,
            &tokens,
        );

        // TTFT should be min of both (100ms - content arrived first)
        assert_eq!(result.ttft, Duration::from_millis(100));
        // TTFO should also be 100ms
        assert_eq!(result.ttfo, Some(Duration::from_millis(100)));
    }
}
