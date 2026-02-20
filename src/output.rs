use crate::benchmark::BenchmarkResult;
use comfy_table::{
    Attribute, Cell, CellAlignment, Color, ContentArrangement, Table, presets::UTF8_FULL_CONDENSED,
};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::{File, create_dir_all};
use std::io::{Error, ErrorKind, Write};
use std::path::{Path, PathBuf};

pub struct BenchmarkConfig<'a> {
    pub model: &'a str,
    pub tokenizer: &'a str,
    pub mean_input_tokens: u32,
    pub stddev_input_tokens: u32,
    pub mean_output_tokens: Option<u32>,
    pub stddev_output_tokens: u32,
    pub num_concurrent_requests: u32,
}

pub struct WrittenResults {
    pub summary: BenchmarkSummary,
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

    pub total_input_tokens: MetricStats,
    pub total_output_tokens: MetricStats,
    pub total_reasoning_tokens: MetricStats,
    pub total_output_sequence_tokens: MetricStats,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub error_summary: Vec<ErrorSummaryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricValue {
    pub value: Value,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestError {
    pub code: i32,
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMetadata {
    pub request_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_start_ns: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_end_ns: Option<u64>,
    pub benchmark_phase: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestRecord {
    pub metadata: RequestMetadata,
    pub metrics: BTreeMap<String, MetricValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RequestError>,
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

fn default_results_dir() -> std::io::Result<PathBuf> {
    let project_dirs = ProjectDirs::from("", "", "llmnop").ok_or_else(|| {
        Error::new(
            ErrorKind::NotFound,
            "could not resolve platform app data directory for llmnop",
        )
    })?;

    if let Some(state_dir) = project_dirs.state_dir() {
        return Ok(state_dir.join("results"));
    }

    Ok(project_dirs.data_local_dir().join("results"))
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

fn generate_run_id() -> std::io::Result<String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::other(format!("system clock is before UNIX_EPOCH: {e}")))?;
    Ok(format!("{}_{:09}", now.as_secs(), now.subsec_nanos()))
}

fn run_results_dir(base_results_dir: &Path, config: &BenchmarkConfig, run_id: &str) -> PathBuf {
    base_results_dir.join(benchmark_slug(config)).join(run_id)
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
        inter_token_vec.push(br.inter_token_latency_s);
        inter_event_vec.push(br.inter_event_latency_s);
        ttft_vec.push(br.ttft.as_secs_f64());
        if let Some(ttfo) = br.ttfo {
            ttfo_vec.push(ttfo.as_secs_f64());
        }
        e2e_vec.push(br.total_latency.as_secs_f64());
        throughput_vec.push(br.throughput);
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
    table.load_preset(UTF8_FULL_CONDENSED);
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

pub fn write_results_json(
    config: &BenchmarkConfig,
    all_results: &[Result<BenchmarkResult, String>],
    total_start_time: std::time::Instant,
    total_end_time: std::time::Instant,
    start_time_unix_ns: u64,
    end_time_unix_ns: u64,
) -> std::io::Result<WrittenResults> {
    let base_results_dir = default_results_dir()?;
    let run_id = generate_run_id()?;
    let run_results_dir = run_results_dir(&base_results_dir, config, &run_id);
    create_dir_all(&run_results_dir)?;

    let mut total_output_tokens = 0_u64;
    let mut total_reasoning_tokens = 0_u64;
    let mut total_input_tokens = 0_u64;
    let mut successful_results = Vec::new();
    let mut error_counts_by_message: BTreeMap<String, usize> = BTreeMap::new();
    let mut per_request_records = Vec::with_capacity(all_results.len());

    for (request_index, result) in all_results.iter().enumerate() {
        match result {
            Ok(br) => {
                total_output_tokens += br.output_tokens as u64;
                total_reasoning_tokens += br.reasoning_tokens as u64;
                total_input_tokens += br.input_tokens as u64;
                successful_results.push(br.clone());

                let mut metrics = BTreeMap::new();
                metrics.insert(
                    "time_to_first_token".to_string(),
                    metric_value_f64(br.ttft.as_secs_f64() * 1000.0, "ms"),
                );
                if let Some(ttfo) = br.ttfo {
                    metrics.insert(
                        "time_to_first_output_token".to_string(),
                        metric_value_f64(ttfo.as_secs_f64() * 1000.0, "ms"),
                    );
                }
                metrics.insert(
                    "request_latency".to_string(),
                    metric_value_f64(br.total_latency.as_secs_f64() * 1000.0, "ms"),
                );
                metrics.insert(
                    "inter_token_latency".to_string(),
                    metric_value_f64(br.inter_token_latency_s * 1000.0, "ms"),
                );
                metrics.insert(
                    "inter_event_latency".to_string(),
                    metric_value_f64(br.inter_event_latency_s * 1000.0, "ms"),
                );
                metrics.insert(
                    "output_token_throughput_per_request".to_string(),
                    metric_value_f64(br.throughput, "tokens/sec/request"),
                );
                metrics.insert(
                    "input_sequence_length".to_string(),
                    metric_value_u64(br.input_tokens as u64, "tokens"),
                );
                metrics.insert(
                    "output_token_count".to_string(),
                    metric_value_u64(br.output_tokens as u64, "tokens"),
                );
                metrics.insert(
                    "reasoning_token_count".to_string(),
                    metric_value_u64(br.reasoning_tokens as u64, "tokens"),
                );
                metrics.insert(
                    "output_sequence_length".to_string(),
                    metric_value_u64((br.output_tokens + br.reasoning_tokens) as u64, "tokens"),
                );

                let record = RequestRecord {
                    metadata: RequestMetadata {
                        request_index,
                        request_start_ns: Some(br.request_start_unix_ns),
                        request_end_ns: Some(br.request_end_unix_ns),
                        benchmark_phase: "profiling".to_string(),
                    },
                    metrics,
                    error: None,
                };
                per_request_records.push(record);
            }
            Err(msg) => {
                *error_counts_by_message.entry(msg.clone()).or_default() += 1;

                let record = RequestRecord {
                    metadata: RequestMetadata {
                        request_index,
                        request_start_ns: None,
                        request_end_ns: None,
                        benchmark_phase: "profiling".to_string(),
                    },
                    metrics: BTreeMap::new(),
                    error: Some(RequestError {
                        code: 1,
                        error_type: "RequestError".to_string(),
                        message: msg.clone(),
                    }),
                };
                per_request_records.push(record);
            }
        }
    }

    {
        let path = run_results_dir.join("individual_responses.jsonl");
        let mut file = File::create(&path)?;
        for record in &per_request_records {
            let line = serde_json::to_string(record)?;
            file.write_all(line.as_bytes())?;
            file.write_all(b"\n")?;
        }
    }

    let summary = build_summary(
        &run_id,
        config,
        &successful_results,
        all_results.len(),
        total_input_tokens,
        total_output_tokens,
        total_reasoning_tokens,
        &error_counts_by_message,
        total_start_time,
        total_end_time,
        start_time_unix_ns,
        end_time_unix_ns,
    );

    {
        let summary_path = run_results_dir.join("summary.json");
        let mut file = File::create(&summary_path)?;
        let summary_json = serde_json::to_string_pretty(&summary)?;
        file.write_all(summary_json.as_bytes())?;
    }

    Ok(WrittenResults { summary })
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

    for br in successful_results {
        request_latency_ms.push(br.total_latency.as_secs_f64() * 1000.0);
        ttft_ms.push(br.ttft.as_secs_f64() * 1000.0);
        if let Some(ttfo) = br.ttfo {
            ttfo_ms.push(ttfo.as_secs_f64() * 1000.0);
        }
        inter_token_ms.push(br.inter_token_latency_s * 1000.0);
        inter_event_ms.push(br.inter_event_latency_s * 1000.0);
        throughput_per_request.push(br.throughput);
        in_tokens.push(br.input_tokens as f64);
        out_tokens.push(br.output_tokens as f64);
        reasoning_tokens.push(br.reasoning_tokens as f64);
        output_sequence_tokens.push((br.output_tokens + br.reasoning_tokens) as f64);
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
        version: "2026-02-19".to_string(),
        schema_version: "2.0".to_string(),
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

fn metric_value_f64(value: f64, unit: &str) -> MetricValue {
    MetricValue {
        value: Value::from(value),
        unit: unit.to_string(),
    }
}

fn metric_value_u64(value: u64, unit: &str) -> MetricValue {
    MetricValue {
        value: Value::from(value),
        unit: unit.to_string(),
    }
}

fn percentile(sorted_values: &[f64], pct: f64) -> f64 {
    if sorted_values.is_empty() {
        return 0.0;
    }
    let idx = ((sorted_values.len() - 1) as f64 * pct).floor() as usize;
    sorted_values[idx]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
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
    fn test_generate_run_id_format() {
        let run_id = generate_run_id().unwrap();
        let mut parts = run_id.split('_');
        let secs = parts.next().unwrap();
        let nanos = parts.next().unwrap();

        assert!(parts.next().is_none());
        assert_eq!(secs.len(), 10);
        assert_eq!(nanos.len(), 9);
        assert!(secs.chars().all(|c| c.is_ascii_digit()));
        assert!(nanos.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_run_results_dir_layout() {
        let config = BenchmarkConfig {
            model: "qwen/qwen3-4b-2507",
            tokenizer: "Qwen/Qwen3-4B",
            mean_input_tokens: 550,
            stddev_input_tokens: 0,
            mean_output_tokens: Some(150),
            stddev_output_tokens: 0,
            num_concurrent_requests: 1,
        };
        let path = run_results_dir(Path::new("/tmp/results"), &config, "1700000000_123456789");
        assert_eq!(
            path,
            PathBuf::from("/tmp/results/qwen-qwen3-4b-2507_550_150/1700000000_123456789")
        );
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
            ttft: Duration::from_millis(100),
            ttfo: Some(Duration::from_millis(120)),
            total_latency: Duration::from_millis(900),
            throughput: 75.0,
            input_tokens: 550,
            output_tokens: 120,
            reasoning_tokens: 30,
            inter_token_latency_s: 0.01,
            inter_event_latency_s: 0.02,
            total_tokens: 700,
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

        assert_eq!(summary.schema_version, "2.0");
        assert_eq!(summary.request_latency.unit, "ms");
        assert_eq!(
            summary.output_token_throughput_per_request.unit,
            "tokens/sec/request"
        );
        assert!(summary.time_to_first_output_token.is_some());
    }
}
