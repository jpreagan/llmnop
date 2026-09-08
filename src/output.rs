use crate::args::Args;
use crate::benchmark::{Phase, RequestRecord, Status, unix_time_ns};
use anyhow::{Context, Result, anyhow};
use comfy_table::{
    Attribute, Cell, CellAlignment, Color, ContentArrangement, Table, presets::UTF8_FULL_CONDENSED,
};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;

pub struct BenchmarkConfig<'a> {
    pub model: &'a str,
    pub tokenizer: &'a str,
    pub mean_input_tokens: u32,
    pub stddev_input_tokens: u32,
    pub mean_output_tokens: Option<u32>,
    pub stddev_output_tokens: u32,
    pub num_concurrent_requests: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryInputConfig {
    pub model: String,
    pub tokenizer: String,
    pub mean_input_tokens: u32,
    pub stddev_input_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stddev_output_tokens: Option<u32>,
    pub num_concurrent_requests: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricStats {
    pub unit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p1: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p5: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p10: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p25: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p50: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p75: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p90: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p95: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p99: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub std: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSummaryEntry {
    pub code: i32,
    pub message: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    pub run_configuration: Option<Value>,
    pub termination: Option<String>,
    pub warmup_attempts: Option<usize>,
    pub version: String,
    pub schema_version: String,
    pub llmnop_version: String,
    pub benchmark_id: String,
    pub benchmark_slug: String,
    pub start_time_unix_ns: u64,
    pub end_time_unix_ns: u64,
    pub input_config: SummaryInputConfig,

    pub benchmark_duration: MetricStats,
    pub request_count: MetricStats,
    pub successful_request_count: MetricStats,
    pub error_request_count: MetricStats,
    pub error_rate: MetricStats,
    pub request_throughput: MetricStats,

    pub request_latency: MetricStats,
    pub time_to_first_token: MetricStats,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_to_first_output_token: Option<MetricStats>,
    pub inter_token_latency: MetricStats,
    pub inter_event_latency: MetricStats,

    pub output_token_throughput_per_request: MetricStats,
    pub output_token_throughput: MetricStats,
    pub total_token_throughput: MetricStats,

    pub input_sequence_length: MetricStats,
    pub output_token_count: MetricStats,
    pub reasoning_token_count: MetricStats,
    pub output_sequence_length: MetricStats,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_input_tokens: Option<MetricStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_output_tokens: Option<MetricStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_total_tokens: Option<MetricStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_reasoning_tokens: Option<MetricStats>,

    pub total_input_tokens: MetricStats,
    pub total_output_tokens: MetricStats,
    pub total_reasoning_tokens: MetricStats,
    pub total_output_sequence_tokens: MetricStats,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub error_summary: Vec<ErrorSummaryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quantiles {
    pub p1: f64,
    pub p5: f64,
    pub p10: f64,
    pub p25: f64,
    pub p50: f64,
    pub p75: f64,
    pub p90: f64,
    pub p95: f64,
    pub p99: f64,
}

fn benchmark_slug(config: &BenchmarkConfig) -> String {
    let output_tokens_str = config
        .mean_output_tokens
        .map(|v| v.to_string())
        .unwrap_or_else(|| "none".to_string());

    format!(
        "{}_{}_{}",
        sanitize_filename::sanitize(config.model.replace(['/', '.'], "-")),
        config.mean_input_tokens,
        output_tokens_str
    )
}

pub fn print_summary_to_stdout(
    successful_results: &[BenchmarkResult],
    num_errors: usize,
    total_output_tokens: u64,
    total_reasoning_tokens: u64,
    start_time: std::time::Instant,
    end_time: std::time::Instant,
) {
    let total_time_s = end_time.duration_since(start_time).as_secs_f64();

    let mut inter_token_vec = Vec::new();
    let mut inter_event_vec = Vec::new();
    let mut ttft_vec = Vec::new();
    let mut ttfo_vec = Vec::new();
    let mut e2e_vec = Vec::new();
    let mut throughput_vec = Vec::new();
    let mut in_tokens_vec = Vec::new();
    let mut reasoning_tokens_vec = Vec::new();
    let mut out_tokens_vec = Vec::new();
    let mut total_tokens_vec = Vec::new();

    for br in successful_results {
        inter_token_vec.extend(br.inter_token_latency_s);
        inter_event_vec.extend(br.inter_event_latency_s);
        ttft_vec.extend(br.ttft.map(|t| t.as_secs_f64()));
        if let Some(ttfo) = br.ttfo {
            ttfo_vec.push(ttfo.as_secs_f64());
        }
        e2e_vec.push(br.total_latency.as_secs_f64());
        throughput_vec.extend(br.throughput);
        in_tokens_vec.push(br.input_tokens as f64);
        reasoning_tokens_vec.push(br.reasoning_tokens as f64);
        out_tokens_vec.push(br.output_tokens as f64);
        total_tokens_vec.push(br.total_tokens as f64);
    }

    let inter_stats = compute_stats(&inter_token_vec);
    let inter_event_stats = compute_stats(&inter_event_vec);
    let ttft_stats = compute_stats(&ttft_vec);
    let ttfo_stats = compute_stats(&ttfo_vec);
    let e2e_stats = compute_stats(&e2e_vec);
    let thr_stats = compute_stats(&throughput_vec);
    let in_stats = compute_stats(&in_tokens_vec);
    let reasoning_stats = compute_stats(&reasoning_tokens_vec);
    let out_stats = compute_stats(&out_tokens_vec);
    let total_stats = compute_stats(&total_tokens_vec);

    let mut table = Table::new();
    table.load_style(UTF8_FULL_CONDENSED);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec![
        Cell::new("Metric").add_attribute(Attribute::Bold),
        Cell::new("avg").add_attribute(Attribute::Bold),
        Cell::new("min").add_attribute(Attribute::Bold),
        Cell::new("max").add_attribute(Attribute::Bold),
        Cell::new("p99").add_attribute(Attribute::Bold),
        Cell::new("p90").add_attribute(Attribute::Bold),
        Cell::new("p50").add_attribute(Attribute::Bold),
        Cell::new("std").add_attribute(Attribute::Bold),
    ]);

    fn fmt_ms(s: f64) -> String {
        format!("{:.2}", s * 1000.0)
    }

    fn fmt_f64(v: f64) -> String {
        format!("{:.2}", v)
    }

    fn fmt_int(v: f64) -> String {
        format!("{}", v as u32)
    }

    fn add_row(table: &mut Table, name: &str, stats: &StatSet, fmt: fn(f64) -> String) {
        table.add_row(vec![
            Cell::new(name).fg(Color::Cyan),
            Cell::new(fmt(stats.mean))
                .set_alignment(CellAlignment::Right)
                .fg(Color::Green),
            Cell::new(fmt(stats.min))
                .set_alignment(CellAlignment::Right)
                .fg(Color::Green),
            Cell::new(fmt(stats.max))
                .set_alignment(CellAlignment::Right)
                .fg(Color::Green),
            Cell::new(fmt(stats.quantiles.p99))
                .set_alignment(CellAlignment::Right)
                .fg(Color::Green),
            Cell::new(fmt(stats.quantiles.p90))
                .set_alignment(CellAlignment::Right)
                .fg(Color::Green),
            Cell::new(fmt(stats.quantiles.p50))
                .set_alignment(CellAlignment::Right)
                .fg(Color::Green),
            Cell::new(fmt(stats.stddev))
                .set_alignment(CellAlignment::Right)
                .fg(Color::Green),
        ]);
    }

    add_row(&mut table, "Inter Token Latency (ms)", &inter_stats, fmt_ms);
    add_row(
        &mut table,
        "Inter Event Latency (ms)",
        &inter_event_stats,
        fmt_ms,
    );
    add_row(&mut table, "Time to First Token (ms)", &ttft_stats, fmt_ms);
    if !ttfo_vec.is_empty() {
        add_row(
            &mut table,
            "Time to First Output Token (ms)",
            &ttfo_stats,
            fmt_ms,
        );
    }
    add_row(&mut table, "End to End Latency (ms)", &e2e_stats, fmt_ms);
    add_row(
        &mut table,
        "Output Throughput Per Request (tokens/s)",
        &thr_stats,
        fmt_f64,
    );
    add_row(&mut table, "Input Tokens", &in_stats, fmt_int);
    if reasoning_stats.max > 0.0 {
        add_row(&mut table, "Reasoning Tokens", &reasoning_stats, fmt_int);
    }
    add_row(&mut table, "Output Tokens", &out_stats, fmt_int);
    add_row(&mut table, "Total Tokens", &total_stats, fmt_int);

    println!();
    println!("{table}");

    let total_generated_tokens = total_output_tokens + total_reasoning_tokens;
    let overall_output_throughput = if total_time_s > 0.0 {
        total_generated_tokens as f64 / total_time_s
    } else {
        0.0
    };

    let num_completed_requests = successful_results.len();
    let completed_requests_per_min = if total_time_s > 0.0 {
        num_completed_requests as f64 / total_time_s * 60.0
    } else {
        0.0
    };

    const CYAN: &str = "\x1b[36m";
    const GREEN: &str = "\x1b[32m";
    const RESET: &str = "\x1b[0m";

    println!();
    println!(
        "{CYAN}Overall Output Throughput:{RESET} {GREEN}{:.2} tokens/s{RESET}",
        overall_output_throughput
    );
    println!(
        "{CYAN}Completed Requests:{RESET} {GREEN}{}{RESET}",
        num_completed_requests
    );
    println!(
        "{CYAN}Requests Per Minute:{RESET} {GREEN}{:.2}{RESET}",
        completed_requests_per_min
    );
    println!("{CYAN}Errors:{RESET} {GREEN}{}{RESET}", num_errors);
}

#[allow(clippy::too_many_arguments)]
fn build_summary(
    run_id: &str,
    config: &BenchmarkConfig,
    successful_results: &[BenchmarkResult],
    num_requests_started: usize,
    total_input_tokens: u64,
    total_output_tokens: u64,
    total_reasoning_tokens: u64,
    error_counts_by_message: &BTreeMap<String, usize>,
    start_time: std::time::Instant,
    end_time: std::time::Instant,
    start_time_unix_ns: u64,
    end_time_unix_ns: u64,
) -> BenchmarkSummary {
    let total_time_s = end_time.duration_since(start_time).as_secs_f64();

    let mut request_latency_ms = Vec::new();
    let mut ttft_ms = Vec::new();
    let mut ttfo_ms = Vec::new();
    let mut inter_token_ms = Vec::new();
    let mut inter_event_ms = Vec::new();
    let mut throughput_per_request = Vec::new();
    let mut in_tokens = Vec::new();
    let mut out_tokens = Vec::new();
    let mut reasoning_tokens = Vec::new();
    let mut output_sequence_tokens = Vec::new();
    let mut usage_input_tokens = Vec::new();
    let mut usage_output_tokens = Vec::new();
    let mut usage_total_tokens = Vec::new();
    let mut usage_reasoning_tokens = Vec::new();

    for br in successful_results {
        request_latency_ms.push(br.total_latency.as_secs_f64() * 1000.0);
        ttft_ms.extend(br.ttft.map(|t| t.as_secs_f64() * 1000.0));
        if let Some(ttfo) = br.ttfo {
            ttfo_ms.push(ttfo.as_secs_f64() * 1000.0);
        }
        inter_token_ms.extend(br.inter_token_latency_s.map(|s| s * 1000.0));
        inter_event_ms.extend(br.inter_event_latency_s.map(|s| s * 1000.0));
        throughput_per_request.extend(br.throughput);
        in_tokens.push(br.input_tokens as f64);
        out_tokens.push(br.output_tokens as f64);
        reasoning_tokens.push(br.reasoning_tokens as f64);
        output_sequence_tokens.push((br.output_tokens + br.reasoning_tokens) as f64);
        if let Some(usage) = &br.provider_usage {
            if let Some(tokens) = usage.input_tokens {
                usage_input_tokens.push(tokens as f64);
            }
            if let Some(tokens) = usage.output_tokens {
                usage_output_tokens.push(tokens as f64);
            }
            if let Some(tokens) = usage.total_tokens {
                usage_total_tokens.push(tokens as f64);
            }
            if let Some(tokens) = usage.reasoning_tokens {
                usage_reasoning_tokens.push(tokens as f64);
            }
        }
    }

    let completed_requests = successful_results.len();
    let num_errors = num_requests_started.saturating_sub(completed_requests);
    let error_rate = if num_requests_started == 0 {
        0.0
    } else {
        num_errors as f64 / num_requests_started as f64
    };

    let request_throughput = if total_time_s > 0.0 {
        completed_requests as f64 / total_time_s
    } else {
        0.0
    };

    let total_output_sequence_tokens = total_output_tokens + total_reasoning_tokens;
    let output_token_throughput = if total_time_s > 0.0 {
        total_output_sequence_tokens as f64 / total_time_s
    } else {
        0.0
    };

    let total_token_throughput = if total_time_s > 0.0 {
        (total_input_tokens + total_output_sequence_tokens) as f64 / total_time_s
    } else {
        0.0
    };

    let error_summary = error_counts_by_message
        .iter()
        .map(|(message, count)| ErrorSummaryEntry {
            code: 1,
            message: message.clone(),
            count: *count,
        })
        .collect();

    BenchmarkSummary {
        run_configuration: None,
        termination: None,
        warmup_attempts: None,
        version: "2026-09-06-lifecycle.1".to_string(),
        schema_version: "2.3".to_string(),
        llmnop_version: env!("CARGO_PKG_VERSION").to_string(),
        benchmark_id: run_id.to_string(),
        benchmark_slug: benchmark_slug(config),
        start_time_unix_ns,
        end_time_unix_ns,
        input_config: SummaryInputConfig {
            model: config.model.to_string(),
            tokenizer: config.tokenizer.to_string(),
            mean_input_tokens: config.mean_input_tokens,
            stddev_input_tokens: config.stddev_input_tokens,
            mean_output_tokens: config.mean_output_tokens,
            stddev_output_tokens: config
                .mean_output_tokens
                .map(|_| config.stddev_output_tokens),
            num_concurrent_requests: config.num_concurrent_requests,
        },
        benchmark_duration: metric_stats_avg_only("sec", total_time_s),
        request_count: metric_stats_avg_only("requests", num_requests_started as f64),
        successful_request_count: metric_stats_avg_only("requests", completed_requests as f64),
        error_request_count: metric_stats_avg_only("requests", num_errors as f64),
        error_rate: metric_stats_avg_only("ratio", error_rate),
        request_throughput: metric_stats_avg_only("requests/sec", request_throughput),
        request_latency: metric_stats_from_values(&request_latency_ms, "ms"),
        time_to_first_token: metric_stats_from_values(&ttft_ms, "ms"),
        time_to_first_output_token: if ttfo_ms.is_empty() {
            None
        } else {
            Some(metric_stats_from_values(&ttfo_ms, "ms"))
        },
        inter_token_latency: metric_stats_from_values(&inter_token_ms, "ms"),
        inter_event_latency: metric_stats_from_values(&inter_event_ms, "ms"),
        output_token_throughput_per_request: metric_stats_from_values(
            &throughput_per_request,
            "tokens/sec/request",
        ),
        output_token_throughput: metric_stats_avg_only("tokens/sec", output_token_throughput),
        total_token_throughput: metric_stats_avg_only("tokens/sec", total_token_throughput),
        input_sequence_length: metric_stats_from_values(&in_tokens, "tokens"),
        output_token_count: metric_stats_from_values(&out_tokens, "tokens"),
        reasoning_token_count: metric_stats_from_values(&reasoning_tokens, "tokens"),
        output_sequence_length: metric_stats_from_values(&output_sequence_tokens, "tokens"),
        usage_input_tokens: metric_stats_optional(&usage_input_tokens, "tokens"),
        usage_output_tokens: metric_stats_optional(&usage_output_tokens, "tokens"),
        usage_total_tokens: metric_stats_optional(&usage_total_tokens, "tokens"),
        usage_reasoning_tokens: metric_stats_optional(&usage_reasoning_tokens, "tokens"),
        total_input_tokens: metric_stats_avg_only("tokens", total_input_tokens as f64),
        total_output_tokens: metric_stats_avg_only("tokens", total_output_tokens as f64),
        total_reasoning_tokens: metric_stats_avg_only("tokens", total_reasoning_tokens as f64),
        total_output_sequence_tokens: metric_stats_avg_only(
            "tokens",
            total_output_sequence_tokens as f64,
        ),
        error_summary,
    }
}

#[derive(Default)]
struct StatSet {
    quantiles: Quantiles,
    mean: f64,
    min: f64,
    max: f64,
    stddev: f64,
}

impl Default for Quantiles {
    fn default() -> Self {
        Self {
            p1: 0.0,
            p5: 0.0,
            p10: 0.0,
            p25: 0.0,
            p50: 0.0,
            p75: 0.0,
            p90: 0.0,
            p95: 0.0,
            p99: 0.0,
        }
    }
}

fn compute_stats(values: &[f64]) -> StatSet {
    if values.is_empty() {
        return StatSet::default();
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
    let min = sorted[0];
    let max = sorted[sorted.len() - 1];
    let stddev = if sorted.len() > 1 {
        let var =
            sorted.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (sorted.len() as f64 - 1.0);
        var.sqrt()
    } else {
        0.0
    };

    let quantiles = Quantiles {
        p1: percentile(&sorted, 0.01),
        p5: percentile(&sorted, 0.05),
        p10: percentile(&sorted, 0.10),
        p25: percentile(&sorted, 0.25),
        p50: percentile(&sorted, 0.50),
        p75: percentile(&sorted, 0.75),
        p90: percentile(&sorted, 0.90),
        p95: percentile(&sorted, 0.95),
        p99: percentile(&sorted, 0.99),
    };

    StatSet {
        quantiles,
        mean,
        min,
        max,
        stddev,
    }
}

fn metric_stats_from_values(values: &[f64], unit: &str) -> MetricStats {
    if values.is_empty() {
        return MetricStats {
            unit: unit.to_string(),
            avg: None,
            p1: None,
            p5: None,
            p10: None,
            p25: None,
            p50: None,
            p75: None,
            p90: None,
            p95: None,
            p99: None,
            min: None,
            max: None,
            std: None,
        };
    }

    let stats = compute_stats(values);
    MetricStats {
        unit: unit.to_string(),
        avg: Some(stats.mean),
        p1: Some(stats.quantiles.p1),
        p5: Some(stats.quantiles.p5),
        p10: Some(stats.quantiles.p10),
        p25: Some(stats.quantiles.p25),
        p50: Some(stats.quantiles.p50),
        p75: Some(stats.quantiles.p75),
        p90: Some(stats.quantiles.p90),
        p95: Some(stats.quantiles.p95),
        p99: Some(stats.quantiles.p99),
        min: Some(stats.min),
        max: Some(stats.max),
        std: Some(stats.stddev),
    }
}

fn metric_stats_optional(values: &[f64], unit: &str) -> Option<MetricStats> {
    if values.is_empty() {
        None
    } else {
        Some(metric_stats_from_values(values, unit))
    }
}

fn metric_stats_avg_only(unit: &str, avg: f64) -> MetricStats {
    MetricStats {
        unit: unit.to_string(),
        avg: Some(avg),
        p1: None,
        p5: None,
        p10: None,
        p25: None,
        p50: None,
        p75: None,
        p90: None,
        p95: None,
        p99: None,
        min: None,
        max: None,
        std: None,
    }
}

fn percentile(sorted_values: &[f64], pct: f64) -> f64 {
    if sorted_values.is_empty() {
        return 0.0;
    }
    let idx = ((sorted_values.len() - 1) as f64 * pct).floor() as usize;
    sorted_values[idx]
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkResult {
    pub ttft: Option<Duration>,
    pub ttfo: Option<Duration>,
    pub total_latency: Duration,
    pub throughput: Option<f64>,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub reasoning_tokens: u32,
    pub inter_token_latency_s: Option<f64>,
    pub inter_event_latency_s: Option<f64>,
    pub total_tokens: u32,
    pub provider_usage: Option<ProviderUsage>,
    pub request_start_unix_ns: u64,
    pub request_end_unix_ns: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderUsage {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub reasoning_tokens: Option<u32>,
}

impl BenchmarkResult {
    pub fn from_record(record: &RequestRecord) -> Result<Self> {
        if record.status != Status::Completed {
            return Err(anyhow!(
                "{}",
                record
                    .error
                    .as_ref()
                    .map(|e| e.message.as_str())
                    .unwrap_or("request failed")
            ));
        }
        let m = &record.metrics;
        let provider_usage = record.provider_usage.as_ref().map(|u| {
            let n = |a: &str, b: &str| {
                u.pointer(a)
                    .or_else(|| u.pointer(b))
                    .and_then(Value::as_u64)
                    .and_then(|n| u32::try_from(n).ok())
            };
            ProviderUsage {
                input_tokens: n("/input_tokens", "/prompt_tokens"),
                output_tokens: n("/output_tokens", "/completion_tokens"),
                total_tokens: n("/total_tokens", "/total_tokens"),
                reasoning_tokens: n(
                    "/output_tokens_details/reasoning_tokens",
                    "/completion_tokens_details/reasoning_tokens",
                ),
            }
        });
        let output_tokens = u32::try_from(m.content_tokens.unwrap())?;
        let reasoning_tokens = u32::try_from(m.reasoning_tokens.unwrap())?;
        let input_tokens = u32::try_from(m.input_tokens)?;
        Ok(Self {
            ttft: m.ttft_ms.map(|n| Duration::from_secs_f64(n / 1000.0)),
            ttfo: m.ttfo_ms.map(|n| Duration::from_secs_f64(n / 1000.0)),
            total_latency: record.end.duration_since(record.start),
            throughput: m.generation_tokens_per_second,
            input_tokens,
            output_tokens,
            reasoning_tokens,
            inter_token_latency_s: m.mean_inter_token_latency_ms.map(|n| n / 1000.0),
            inter_event_latency_s: m.mean_inter_event_latency_ms.map(|n| n / 1000.0),
            total_tokens: input_tokens
                .checked_add(output_tokens)
                .and_then(|n| n.checked_add(reasoning_tokens))
                .ok_or_else(|| anyhow!("token total overflow"))?,
            provider_usage,
            request_start_unix_ns: record.start_time_unix_ns,
            request_end_unix_ns: record.end_time_unix_ns,
        })
    }
}

impl BenchmarkSummary {
    pub fn new(args: &Args, run_id: String, records: &[RequestRecord], interrupted: bool) -> Self {
        let measured: Vec<_> = records
            .iter()
            .filter(|r| r.phase == Phase::Measurement)
            .collect();
        let successful: Vec<_> = measured
            .iter()
            .filter_map(|r| BenchmarkResult::from_record(r).ok())
            .collect();
        let mut errors = BTreeMap::new();
        for record in &measured {
            if let Some(error) = &record.error {
                *errors.entry(error.message.clone()).or_default() += 1;
            }
        }
        let now = Instant::now();
        let start = measured.iter().min_by_key(|r| r.start);
        let end = measured.iter().max_by_key(|r| r.end);
        let config = BenchmarkConfig {
            model: args.model.as_deref().unwrap(),
            tokenizer: args.tokenizer.as_deref().unwrap(),
            mean_input_tokens: args.input_tokens,
            stddev_input_tokens: args.input_tokens_stddev,
            mean_output_tokens: args.output_cap,
            stddev_output_tokens: args.output_cap_stddev,
            num_concurrent_requests: args.concurrency,
        };
        let mut summary = build_summary(
            &run_id,
            &config,
            &successful,
            measured.len(),
            successful.iter().map(|r| u64::from(r.input_tokens)).sum(),
            successful.iter().map(|r| u64::from(r.output_tokens)).sum(),
            successful
                .iter()
                .map(|r| u64::from(r.reasoning_tokens))
                .sum(),
            &errors,
            start.map_or(now, |r| r.start),
            end.map_or(now, |r| r.end),
            start.map_or(0, |r| r.start_time_unix_ns),
            end.map_or(0, |r| r.end_time_unix_ns),
        );
        summary.run_configuration =
            Some(serde_json::to_value(args).expect("validated configuration is serializable"));
        summary.termination = Some(
            if interrupted {
                "interrupted"
            } else {
                "request_count"
            }
            .into(),
        );
        summary.warmup_attempts = Some(records.iter().filter(|r| r.phase == Phase::Warmup).count());
        summary
    }
}

pub fn print_records(records: &[RequestRecord]) {
    let measured: Vec<_> = records
        .iter()
        .filter(|r| r.phase == Phase::Measurement)
        .collect();
    let successful: Vec<_> = measured
        .iter()
        .filter_map(|r| BenchmarkResult::from_record(r).ok())
        .collect();
    let now = Instant::now();
    print_summary_to_stdout(
        &successful,
        measured.len() - successful.len(),
        successful.iter().map(|r| u64::from(r.output_tokens)).sum(),
        successful
            .iter()
            .map(|r| u64::from(r.reasoning_tokens))
            .sum(),
        measured.iter().map(|r| r.start).min().unwrap_or(now),
        measured.iter().map(|r| r.end).max().unwrap_or(now),
    );
}
pub struct ResultsWriter {
    pub directory: PathBuf,
    pub run_id: String,
    records: tokio::fs::File,
}

impl ResultsWriter {
    pub async fn new(parent: Option<&Path>) -> Result<Self> {
        let parent = match parent {
            Some(path) => path.to_owned(),
            None => {
                let dirs = ProjectDirs::from("", "", "llmnop")
                    .context("could not locate results directory")?;
                dirs.state_dir()
                    .unwrap_or_else(|| dirs.data_local_dir())
                    .join("results")
            }
        };
        tokio::fs::create_dir_all(&parent).await?;
        let run_id = format!("{}_{}", unix_time_ns(), std::process::id());
        let directory = parent.join(&run_id);
        tokio::fs::create_dir(&directory)
            .await
            .context("could not create run directory")?;
        let records = tokio::fs::File::create_new(directory.join("requests.jsonl")).await?;
        Ok(Self {
            directory,
            run_id,
            records,
        })
    }

    pub async fn append(&mut self, record: &RequestRecord) -> Result<()> {
        let mut line = serde_json::to_vec(record)?;
        line.push(b'\n');
        self.records.write_all(&line).await?;
        self.records.flush().await?;
        Ok(())
    }

    pub async fn finish(&mut self, summary: &BenchmarkSummary) -> Result<()> {
        self.records.flush().await?;
        let path = self.directory.join("summary.json");
        tokio::fs::write(path, serde_json::to_vec_pretty(summary)?).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_percentile_calculation() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        assert_eq!(percentile(&values, 0.0), 1.0);
        assert_eq!(percentile(&values, 0.25), 2.0);
        assert_eq!(percentile(&values, 0.5), 3.0);
        assert_eq!(percentile(&values, 0.75), 4.0);
        assert_eq!(percentile(&values, 1.0), 5.0);

        assert_eq!(percentile(&[], 0.5), 0.0);
        assert_eq!(percentile(&[42.0], 0.5), 42.0);
    }

    #[test]
    fn test_quantiles_serialization() {
        let quantiles = Quantiles {
            p1: 0.01,
            p5: 0.05,
            p10: 0.1,
            p25: 0.25,
            p50: 0.5,
            p75: 0.75,
            p90: 0.9,
            p95: 0.95,
            p99: 0.99,
        };

        let json = serde_json::to_string(&quantiles).unwrap();
        let deserialized: Quantiles = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.p10, 0.1);
        assert_eq!(deserialized.p50, 0.5);
        assert_eq!(deserialized.p99, 0.99);
    }

    #[test]
    fn test_stats_computation() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let stats = compute_stats(&values);

        assert_eq!(stats.mean, 3.0);
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 5.0);
        assert!(stats.stddev > 0.0);

        assert!(stats.min <= stats.quantiles.p25);
        assert!(stats.quantiles.p25 <= stats.quantiles.p50);
        assert!(stats.quantiles.p75 <= stats.max);

        let empty_stats = compute_stats(&[]);
        assert_eq!(empty_stats.min, 0.0);
        assert_eq!(empty_stats.max, 0.0);
        assert_eq!(empty_stats.mean, 0.0);
    }

    #[test]
    fn test_metric_stats_avg_only_serialization() {
        let metric = metric_stats_avg_only("requests", 10.0);
        let value = serde_json::to_value(metric).unwrap();

        assert_eq!(value.get("unit").and_then(Value::as_str), Some("requests"));
        assert_eq!(value.get("avg").and_then(Value::as_f64), Some(10.0));
        assert!(value.get("p99").is_none());
    }

    #[test]
    fn test_benchmark_slug() {
        let config = BenchmarkConfig {
            model: "qwen/qwen3-4b-2507",
            tokenizer: "Qwen/Qwen3-4B",
            mean_input_tokens: 550,
            stddev_input_tokens: 0,
            mean_output_tokens: Some(150),
            stddev_output_tokens: 0,
            num_concurrent_requests: 1,
        };

        assert_eq!(benchmark_slug(&config), "qwen-qwen3-4b-2507_550_150");
    }

    #[test]
    fn test_benchmark_slug_without_output_tokens() {
        let config = BenchmarkConfig {
            model: "qwen/qwen3-4b-2507",
            tokenizer: "Qwen/Qwen3-4B",
            mean_input_tokens: 550,
            stddev_input_tokens: 0,
            mean_output_tokens: None,
            stddev_output_tokens: 0,
            num_concurrent_requests: 1,
        };

        assert_eq!(benchmark_slug(&config), "qwen-qwen3-4b-2507_550_none");
    }

    #[test]
    fn test_build_summary_has_nested_metrics() {
        let config = BenchmarkConfig {
            model: "qwen/qwen3-4b-2507",
            tokenizer: "Qwen/Qwen3-4B",
            mean_input_tokens: 550,
            stddev_input_tokens: 0,
            mean_output_tokens: Some(150),
            stddev_output_tokens: 0,
            num_concurrent_requests: 1,
        };

        let successful_results = vec![BenchmarkResult {
            ttft: Some(Duration::from_millis(100)),
            ttfo: Some(Duration::from_millis(120)),
            total_latency: Duration::from_millis(900),
            throughput: Some(75.0),
            input_tokens: 550,
            output_tokens: 120,
            reasoning_tokens: 30,
            inter_token_latency_s: Some(0.01),
            inter_event_latency_s: Some(0.02),
            total_tokens: 700,
            provider_usage: Some(ProviderUsage {
                input_tokens: Some(550),
                output_tokens: Some(150),
                total_tokens: Some(700),
                reasoning_tokens: Some(30),
            }),
            request_start_unix_ns: 1_700_000_000_000_000_000,
            request_end_unix_ns: 1_700_000_000_900_000_000,
        }];

        let summary = build_summary(
            "1700000000_123456789",
            &config,
            &successful_results,
            1,
            550,
            120,
            30,
            &BTreeMap::new(),
            std::time::Instant::now(),
            std::time::Instant::now() + Duration::from_secs(1),
            1_700_000_000_000_000_000,
            1_700_000_001_000_000_000,
        );

        assert_eq!(summary.schema_version, "2.3");
        assert_eq!(summary.version, "2026-09-06-lifecycle.1");
        assert_eq!(summary.request_latency.unit, "ms");
        assert_eq!(
            summary.output_token_throughput_per_request.unit,
            "tokens/sec/request"
        );
        assert!(summary.time_to_first_output_token.is_some());
        assert_eq!(
            summary.usage_output_tokens.and_then(|stats| stats.avg),
            Some(150.0)
        );
    }
}
