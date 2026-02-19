use crate::benchmark::BenchmarkResult;
use comfy_table::{
    Attribute, Cell, CellAlignment, Color, ContentArrangement, Table, presets::UTF8_FULL_CONDENSED,
};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs::{File, create_dir_all};
use std::io::Write;
use std::path::PathBuf;

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
pub struct IndividualResponse {
    #[serde(rename = "error_code")]
    pub error_code: Option<i32>,

    #[serde(rename = "error_msg")]
    pub error_msg: String,

    #[serde(rename = "inter_token_latency_s")]
    pub inter_token_latency_s: Option<f64>,

    #[serde(rename = "inter_event_latency_s")]
    pub inter_event_latency_s: Option<f64>,

    #[serde(rename = "ttft_s")]
    pub ttft_s: Option<f64>,

    #[serde(rename = "ttfo_s")]
    pub ttfo_s: Option<f64>,

    #[serde(rename = "end_to_end_latency_s")]
    pub end_to_end_latency_s: Option<f64>,

    #[serde(rename = "request_output_throughput_token_per_s")]
    pub request_output_throughput_token_per_s: Option<f64>,

    #[serde(rename = "number_input_tokens")]
    pub number_input_tokens: Option<u32>,

    #[serde(rename = "number_reasoning_tokens")]
    pub number_reasoning_tokens: Option<u32>,

    #[serde(rename = "number_output_tokens")]
    pub number_output_tokens: Option<u32>,

    #[serde(rename = "number_total_tokens")]
    pub number_total_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    pub version: String,
    pub name: String,
    pub model: String,
    pub tokenizer: String,
    pub mean_input_tokens: u32,
    pub stddev_input_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stddev_output_tokens: Option<u32>,
    pub num_concurrent_requests: u32,

    pub results_inter_token_latency_s_quantiles_p25: f64,
    pub results_inter_token_latency_s_quantiles_p50: f64,
    pub results_inter_token_latency_s_quantiles_p75: f64,
    pub results_inter_token_latency_s_quantiles_p90: f64,
    pub results_inter_token_latency_s_quantiles_p95: f64,
    pub results_inter_token_latency_s_quantiles_p99: f64,
    pub results_inter_token_latency_s_mean: f64,
    pub results_inter_token_latency_s_min: f64,
    pub results_inter_token_latency_s_max: f64,
    pub results_inter_token_latency_s_stddev: f64,

    pub results_inter_event_latency_s_quantiles_p25: f64,
    pub results_inter_event_latency_s_quantiles_p50: f64,
    pub results_inter_event_latency_s_quantiles_p75: f64,
    pub results_inter_event_latency_s_quantiles_p90: f64,
    pub results_inter_event_latency_s_quantiles_p95: f64,
    pub results_inter_event_latency_s_quantiles_p99: f64,
    pub results_inter_event_latency_s_mean: f64,
    pub results_inter_event_latency_s_min: f64,
    pub results_inter_event_latency_s_max: f64,
    pub results_inter_event_latency_s_stddev: f64,

    pub results_ttft_s_quantiles_p25: f64,
    pub results_ttft_s_quantiles_p50: f64,
    pub results_ttft_s_quantiles_p75: f64,
    pub results_ttft_s_quantiles_p90: f64,
    pub results_ttft_s_quantiles_p95: f64,
    pub results_ttft_s_quantiles_p99: f64,
    pub results_ttft_s_mean: f64,
    pub results_ttft_s_min: f64,
    pub results_ttft_s_max: f64,
    pub results_ttft_s_stddev: f64,

    pub results_ttfo_s_quantiles_p25: Option<f64>,
    pub results_ttfo_s_quantiles_p50: Option<f64>,
    pub results_ttfo_s_quantiles_p75: Option<f64>,
    pub results_ttfo_s_quantiles_p90: Option<f64>,
    pub results_ttfo_s_quantiles_p95: Option<f64>,
    pub results_ttfo_s_quantiles_p99: Option<f64>,
    pub results_ttfo_s_mean: Option<f64>,
    pub results_ttfo_s_min: Option<f64>,
    pub results_ttfo_s_max: Option<f64>,
    pub results_ttfo_s_stddev: Option<f64>,

    pub results_end_to_end_latency_s_quantiles_p25: f64,
    pub results_end_to_end_latency_s_quantiles_p50: f64,
    pub results_end_to_end_latency_s_quantiles_p75: f64,
    pub results_end_to_end_latency_s_quantiles_p90: f64,
    pub results_end_to_end_latency_s_quantiles_p95: f64,
    pub results_end_to_end_latency_s_quantiles_p99: f64,
    pub results_end_to_end_latency_s_mean: f64,
    pub results_end_to_end_latency_s_min: f64,
    pub results_end_to_end_latency_s_max: f64,
    pub results_end_to_end_latency_s_stddev: f64,

    pub results_request_output_throughput_token_per_s_quantiles_p25: f64,
    pub results_request_output_throughput_token_per_s_quantiles_p50: f64,
    pub results_request_output_throughput_token_per_s_quantiles_p75: f64,
    pub results_request_output_throughput_token_per_s_quantiles_p90: f64,
    pub results_request_output_throughput_token_per_s_quantiles_p95: f64,
    pub results_request_output_throughput_token_per_s_quantiles_p99: f64,
    pub results_request_output_throughput_token_per_s_mean: f64,
    pub results_request_output_throughput_token_per_s_min: f64,
    pub results_request_output_throughput_token_per_s_max: f64,
    pub results_request_output_throughput_token_per_s_stddev: f64,

    pub results_number_input_tokens_quantiles_p25: f64,
    pub results_number_input_tokens_quantiles_p50: f64,
    pub results_number_input_tokens_quantiles_p75: f64,
    pub results_number_input_tokens_quantiles_p90: f64,
    pub results_number_input_tokens_quantiles_p95: f64,
    pub results_number_input_tokens_quantiles_p99: f64,
    pub results_number_input_tokens_mean: f64,
    pub results_number_input_tokens_min: String,
    pub results_number_input_tokens_max: String,
    pub results_number_input_tokens_stddev: f64,

    pub results_number_reasoning_tokens_quantiles_p25: f64,
    pub results_number_reasoning_tokens_quantiles_p50: f64,
    pub results_number_reasoning_tokens_quantiles_p75: f64,
    pub results_number_reasoning_tokens_quantiles_p90: f64,
    pub results_number_reasoning_tokens_quantiles_p95: f64,
    pub results_number_reasoning_tokens_quantiles_p99: f64,
    pub results_number_reasoning_tokens_mean: f64,
    pub results_number_reasoning_tokens_min: String,
    pub results_number_reasoning_tokens_max: String,
    pub results_number_reasoning_tokens_stddev: f64,

    pub results_number_output_tokens_quantiles_p25: f64,
    pub results_number_output_tokens_quantiles_p50: f64,
    pub results_number_output_tokens_quantiles_p75: f64,
    pub results_number_output_tokens_quantiles_p90: f64,
    pub results_number_output_tokens_quantiles_p95: f64,
    pub results_number_output_tokens_quantiles_p99: f64,
    pub results_number_output_tokens_mean: f64,
    pub results_number_output_tokens_min: String,
    pub results_number_output_tokens_max: String,
    pub results_number_output_tokens_stddev: f64,

    pub results_number_total_tokens_quantiles_p25: f64,
    pub results_number_total_tokens_quantiles_p50: f64,
    pub results_number_total_tokens_quantiles_p75: f64,
    pub results_number_total_tokens_quantiles_p90: f64,
    pub results_number_total_tokens_quantiles_p95: f64,
    pub results_number_total_tokens_quantiles_p99: f64,
    pub results_number_total_tokens_mean: f64,
    pub results_number_total_tokens_min: String,
    pub results_number_total_tokens_max: String,
    pub results_number_total_tokens_stddev: f64,

    pub results_num_requests_started: usize,
    pub results_error_rate: f64,
    pub results_number_errors: usize,
    pub results_error_code_frequency: String,
    pub results_mean_output_throughput_token_per_s: f64,
    pub results_num_completed_requests: usize,
    pub results_num_completed_requests_per_min: f64,

    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quantiles {
    pub p25: f64,
    pub p50: f64,
    pub p75: f64,
    pub p90: f64,
    pub p95: f64,
    pub p99: f64,
}

fn default_results_dir() -> PathBuf {
    if let Some(project_dirs) = ProjectDirs::from("", "", "llmnop") {
        if let Some(state_dir) = project_dirs.state_dir() {
            return state_dir.join("results");
        }
        return project_dirs.data_local_dir().join("results");
    }

    // Keep a local fallback for unusual environments that do not expose user dirs.
    PathBuf::from("result_outputs")
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

    let inter_stats = compute_stats_for_flatten(&inter_token_vec);
    let inter_event_stats = compute_stats_for_flatten(&inter_event_vec);
    let ttft_stats = compute_stats_for_flatten(&ttft_vec);
    let ttfo_stats = compute_stats_for_flatten(&ttfo_vec);
    let e2e_stats = compute_stats_for_flatten(&e2e_vec);
    let thr_stats = compute_stats_for_flatten(&throughput_vec);
    let in_stats = compute_stats_for_flatten(&in_tokens_vec);
    let reasoning_stats = compute_stats_for_flatten(&reasoning_tokens_vec);
    let out_stats = compute_stats_for_flatten(&out_tokens_vec);
    let total_stats = compute_stats_for_flatten(&total_tokens_vec);

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
) -> std::io::Result<()> {
    let results_dir = default_results_dir();
    create_dir_all(&results_dir)?;

    let mut individual_responses = Vec::with_capacity(all_results.len());
    let mut total_output_tokens = 0_u64;
    let mut total_reasoning_tokens = 0_u64;
    let mut successful_results = Vec::new();

    for result in all_results {
        match result {
            Ok(br) => {
                total_output_tokens += br.output_tokens as u64;
                total_reasoning_tokens += br.reasoning_tokens as u64;

                successful_results.push(br.clone());
                let rec = IndividualResponse {
                    error_code: None,
                    error_msg: "".to_string(),
                    inter_token_latency_s: Some(br.inter_token_latency_s),
                    inter_event_latency_s: Some(br.inter_event_latency_s),
                    ttft_s: Some(br.ttft.as_secs_f64()),
                    ttfo_s: br.ttfo.map(|d| d.as_secs_f64()),
                    end_to_end_latency_s: Some(br.total_latency.as_secs_f64()),
                    request_output_throughput_token_per_s: Some(br.throughput),
                    number_total_tokens: Some(br.total_tokens),
                    number_output_tokens: Some(br.output_tokens),
                    number_reasoning_tokens: Some(br.reasoning_tokens),
                    number_input_tokens: Some(br.input_tokens),
                };
                individual_responses.push(rec);
            }
            Err(msg) => {
                let rec = IndividualResponse {
                    error_code: Some(1),
                    error_msg: msg.clone(),
                    inter_token_latency_s: None,
                    inter_event_latency_s: None,
                    ttft_s: None,
                    ttfo_s: None,
                    end_to_end_latency_s: None,
                    request_output_throughput_token_per_s: None,
                    number_total_tokens: None,
                    number_output_tokens: None,
                    number_reasoning_tokens: None,
                    number_input_tokens: None,
                };
                individual_responses.push(rec);
            }
        }
    }

    {
        let output_tokens_str = config
            .mean_output_tokens
            .map(|v| v.to_string())
            .unwrap_or_else(|| "none".to_string());
        let file_name = format!(
            "{}_{}_{}_individual_responses.json",
            sanitize_filename::sanitize(config.model.replace(['/', '.'], "-")),
            config.mean_input_tokens,
            output_tokens_str
        );

        let path = results_dir.join(file_name);
        let mut f = File::create(&path)?;
        let resp_json = serde_json::to_string_pretty(&individual_responses)?;
        f.write_all(resp_json.as_bytes())?;
    }

    {
        let output_tokens_str = config
            .mean_output_tokens
            .map(|v| v.to_string())
            .unwrap_or_else(|| "none".to_string());
        let summary_filename = format!(
            "{}_{}_{}_summary.json",
            sanitize_filename::sanitize(config.model.replace(['/', '.'], "-")),
            config.mean_input_tokens,
            output_tokens_str
        );
        let summary_path = results_dir.join(summary_filename);

        let flattened = build_flattened_summary(
            config,
            &successful_results,
            all_results.len(),
            all_results.iter().filter(|r| r.is_err()).count(),
            total_output_tokens,
            total_reasoning_tokens,
            total_start_time,
            total_end_time,
        );

        let mut sf = File::create(&summary_path)?;
        let summary_json = serde_json::to_string_pretty(&flattened)?;
        sf.write_all(summary_json.as_bytes())?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_flattened_summary(
    config: &BenchmarkConfig,
    successful_results: &[BenchmarkResult],
    num_requests_started: usize,
    num_errors: usize,
    total_output_tokens: u64,
    total_reasoning_tokens: u64,
    start_time: std::time::Instant,
    end_time: std::time::Instant,
) -> BenchmarkSummary {
    use std::time::{SystemTime, UNIX_EPOCH};

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

    let inter_stats = compute_stats_for_flatten(&inter_token_vec);
    let inter_event_stats = compute_stats_for_flatten(&inter_event_vec);
    let ttft_stats = compute_stats_for_flatten(&ttft_vec);
    let ttfo_stats = compute_stats_for_flatten(&ttfo_vec);
    let e2e_stats = compute_stats_for_flatten(&e2e_vec);
    let thr_stats = compute_stats_for_flatten(&throughput_vec);
    let in_stats = compute_stats_for_flatten(&in_tokens_vec);
    let reasoning_stats = compute_stats_for_flatten(&reasoning_tokens_vec);
    let out_stats = compute_stats_for_flatten(&out_tokens_vec);
    let total_stats = compute_stats_for_flatten(&total_tokens_vec);

    let error_rate = if num_requests_started == 0 {
        0.0
    } else {
        num_errors as f64 / num_requests_started as f64
    };

    let error_code_frequency = if num_errors > 0 {
        format!("{{\"1\": {}}}", num_errors)
    } else {
        "{}".to_string()
    };

    let total_generated_tokens = total_output_tokens + total_reasoning_tokens;
    let mean_output_throughput_token_per_s = if total_time_s > 0.0 {
        total_generated_tokens as f64 / total_time_s
    } else {
        0.0
    };
    let num_completed_requests = successful_results.len();
    let num_completed_requests_per_min = if total_time_s > 0.0 {
        num_completed_requests as f64 / total_time_s * 60.0
    } else {
        0.0
    };

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let (
        ttfo_p25,
        ttfo_p50,
        ttfo_p75,
        ttfo_p90,
        ttfo_p95,
        ttfo_p99,
        ttfo_mean,
        ttfo_min,
        ttfo_max,
        ttfo_stddev,
    ) = if ttfo_vec.is_empty() {
        (None, None, None, None, None, None, None, None, None, None)
    } else {
        (
            Some(ttfo_stats.quantiles.p25),
            Some(ttfo_stats.quantiles.p50),
            Some(ttfo_stats.quantiles.p75),
            Some(ttfo_stats.quantiles.p90),
            Some(ttfo_stats.quantiles.p95),
            Some(ttfo_stats.quantiles.p99),
            Some(ttfo_stats.mean),
            Some(ttfo_stats.min),
            Some(ttfo_stats.max),
            Some(ttfo_stats.stddev),
        )
    };

    let output_tokens_str = config
        .mean_output_tokens
        .map(|v| v.to_string())
        .unwrap_or_else(|| "none".to_string());

    BenchmarkSummary {
        version: "2026-02-19".to_string(),
        name: format!(
            "{}_{}_{}_summary",
            sanitize_filename::sanitize(config.model.replace(['/', '.'], "-")),
            config.mean_input_tokens,
            output_tokens_str
        ),
        model: config.model.to_string(),
        tokenizer: config.tokenizer.to_string(),
        mean_input_tokens: config.mean_input_tokens,
        stddev_input_tokens: config.stddev_input_tokens,
        mean_output_tokens: config.mean_output_tokens,
        stddev_output_tokens: config
            .mean_output_tokens
            .map(|_| config.stddev_output_tokens),
        num_concurrent_requests: config.num_concurrent_requests,

        results_inter_token_latency_s_quantiles_p25: inter_stats.quantiles.p25,
        results_inter_token_latency_s_quantiles_p50: inter_stats.quantiles.p50,
        results_inter_token_latency_s_quantiles_p75: inter_stats.quantiles.p75,
        results_inter_token_latency_s_quantiles_p90: inter_stats.quantiles.p90,
        results_inter_token_latency_s_quantiles_p95: inter_stats.quantiles.p95,
        results_inter_token_latency_s_quantiles_p99: inter_stats.quantiles.p99,
        results_inter_token_latency_s_mean: inter_stats.mean,
        results_inter_token_latency_s_min: inter_stats.min,
        results_inter_token_latency_s_max: inter_stats.max,
        results_inter_token_latency_s_stddev: inter_stats.stddev,

        results_inter_event_latency_s_quantiles_p25: inter_event_stats.quantiles.p25,
        results_inter_event_latency_s_quantiles_p50: inter_event_stats.quantiles.p50,
        results_inter_event_latency_s_quantiles_p75: inter_event_stats.quantiles.p75,
        results_inter_event_latency_s_quantiles_p90: inter_event_stats.quantiles.p90,
        results_inter_event_latency_s_quantiles_p95: inter_event_stats.quantiles.p95,
        results_inter_event_latency_s_quantiles_p99: inter_event_stats.quantiles.p99,
        results_inter_event_latency_s_mean: inter_event_stats.mean,
        results_inter_event_latency_s_min: inter_event_stats.min,
        results_inter_event_latency_s_max: inter_event_stats.max,
        results_inter_event_latency_s_stddev: inter_event_stats.stddev,

        results_ttft_s_quantiles_p25: ttft_stats.quantiles.p25,
        results_ttft_s_quantiles_p50: ttft_stats.quantiles.p50,
        results_ttft_s_quantiles_p75: ttft_stats.quantiles.p75,
        results_ttft_s_quantiles_p90: ttft_stats.quantiles.p90,
        results_ttft_s_quantiles_p95: ttft_stats.quantiles.p95,
        results_ttft_s_quantiles_p99: ttft_stats.quantiles.p99,
        results_ttft_s_mean: ttft_stats.mean,
        results_ttft_s_min: ttft_stats.min,
        results_ttft_s_max: ttft_stats.max,
        results_ttft_s_stddev: ttft_stats.stddev,

        results_ttfo_s_quantiles_p25: ttfo_p25,
        results_ttfo_s_quantiles_p50: ttfo_p50,
        results_ttfo_s_quantiles_p75: ttfo_p75,
        results_ttfo_s_quantiles_p90: ttfo_p90,
        results_ttfo_s_quantiles_p95: ttfo_p95,
        results_ttfo_s_quantiles_p99: ttfo_p99,
        results_ttfo_s_mean: ttfo_mean,
        results_ttfo_s_min: ttfo_min,
        results_ttfo_s_max: ttfo_max,
        results_ttfo_s_stddev: ttfo_stddev,

        results_end_to_end_latency_s_quantiles_p25: e2e_stats.quantiles.p25,
        results_end_to_end_latency_s_quantiles_p50: e2e_stats.quantiles.p50,
        results_end_to_end_latency_s_quantiles_p75: e2e_stats.quantiles.p75,
        results_end_to_end_latency_s_quantiles_p90: e2e_stats.quantiles.p90,
        results_end_to_end_latency_s_quantiles_p95: e2e_stats.quantiles.p95,
        results_end_to_end_latency_s_quantiles_p99: e2e_stats.quantiles.p99,
        results_end_to_end_latency_s_mean: e2e_stats.mean,
        results_end_to_end_latency_s_min: e2e_stats.min,
        results_end_to_end_latency_s_max: e2e_stats.max,
        results_end_to_end_latency_s_stddev: e2e_stats.stddev,

        results_request_output_throughput_token_per_s_quantiles_p25: thr_stats.quantiles.p25,
        results_request_output_throughput_token_per_s_quantiles_p50: thr_stats.quantiles.p50,
        results_request_output_throughput_token_per_s_quantiles_p75: thr_stats.quantiles.p75,
        results_request_output_throughput_token_per_s_quantiles_p90: thr_stats.quantiles.p90,
        results_request_output_throughput_token_per_s_quantiles_p95: thr_stats.quantiles.p95,
        results_request_output_throughput_token_per_s_quantiles_p99: thr_stats.quantiles.p99,
        results_request_output_throughput_token_per_s_mean: thr_stats.mean,
        results_request_output_throughput_token_per_s_min: thr_stats.min,
        results_request_output_throughput_token_per_s_max: thr_stats.max,
        results_request_output_throughput_token_per_s_stddev: thr_stats.stddev,

        results_number_input_tokens_quantiles_p25: in_stats.quantiles.p25,
        results_number_input_tokens_quantiles_p50: in_stats.quantiles.p50,
        results_number_input_tokens_quantiles_p75: in_stats.quantiles.p75,
        results_number_input_tokens_quantiles_p90: in_stats.quantiles.p90,
        results_number_input_tokens_quantiles_p95: in_stats.quantiles.p95,
        results_number_input_tokens_quantiles_p99: in_stats.quantiles.p99,
        results_number_input_tokens_mean: in_stats.mean,
        results_number_input_tokens_min: format!("{}", in_stats.min as u32),
        results_number_input_tokens_max: format!("{}", in_stats.max as u32),
        results_number_input_tokens_stddev: in_stats.stddev,

        results_number_reasoning_tokens_quantiles_p25: reasoning_stats.quantiles.p25,
        results_number_reasoning_tokens_quantiles_p50: reasoning_stats.quantiles.p50,
        results_number_reasoning_tokens_quantiles_p75: reasoning_stats.quantiles.p75,
        results_number_reasoning_tokens_quantiles_p90: reasoning_stats.quantiles.p90,
        results_number_reasoning_tokens_quantiles_p95: reasoning_stats.quantiles.p95,
        results_number_reasoning_tokens_quantiles_p99: reasoning_stats.quantiles.p99,
        results_number_reasoning_tokens_mean: reasoning_stats.mean,
        results_number_reasoning_tokens_min: format!("{}", reasoning_stats.min as u32),
        results_number_reasoning_tokens_max: format!("{}", reasoning_stats.max as u32),
        results_number_reasoning_tokens_stddev: reasoning_stats.stddev,

        results_number_output_tokens_quantiles_p25: out_stats.quantiles.p25,
        results_number_output_tokens_quantiles_p50: out_stats.quantiles.p50,
        results_number_output_tokens_quantiles_p75: out_stats.quantiles.p75,
        results_number_output_tokens_quantiles_p90: out_stats.quantiles.p90,
        results_number_output_tokens_quantiles_p95: out_stats.quantiles.p95,
        results_number_output_tokens_quantiles_p99: out_stats.quantiles.p99,
        results_number_output_tokens_mean: out_stats.mean,
        results_number_output_tokens_min: format!("{}", out_stats.min as u32),
        results_number_output_tokens_max: format!("{}", out_stats.max as u32),
        results_number_output_tokens_stddev: out_stats.stddev,

        results_number_total_tokens_quantiles_p25: total_stats.quantiles.p25,
        results_number_total_tokens_quantiles_p50: total_stats.quantiles.p50,
        results_number_total_tokens_quantiles_p75: total_stats.quantiles.p75,
        results_number_total_tokens_quantiles_p90: total_stats.quantiles.p90,
        results_number_total_tokens_quantiles_p95: total_stats.quantiles.p95,
        results_number_total_tokens_quantiles_p99: total_stats.quantiles.p99,
        results_number_total_tokens_mean: total_stats.mean,
        results_number_total_tokens_min: format!("{}", total_stats.min as u32),
        results_number_total_tokens_max: format!("{}", total_stats.max as u32),
        results_number_total_tokens_stddev: total_stats.stddev,

        results_num_requests_started: num_requests_started,
        results_error_rate: error_rate,
        results_number_errors: num_errors,
        results_error_code_frequency: error_code_frequency,
        results_mean_output_throughput_token_per_s: mean_output_throughput_token_per_s,
        results_num_completed_requests: num_completed_requests,
        results_num_completed_requests_per_min: num_completed_requests_per_min,

        timestamp,
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
            p25: 0.0,
            p50: 0.0,
            p75: 0.0,
            p90: 0.0,
            p95: 0.0,
            p99: 0.0,
        }
    }
}

fn compute_stats_for_flatten(values: &[f64]) -> StatSet {
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
            p25: 0.1,
            p50: 0.2,
            p75: 0.3,
            p90: 0.4,
            p95: 0.5,
            p99: 0.6,
        };

        let json = serde_json::to_string(&quantiles).unwrap();
        let deserialized: Quantiles = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.p25, 0.1);
        assert_eq!(deserialized.p50, 0.2);
    }

    #[test]
    fn test_stats_computation() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let stats = compute_stats_for_flatten(&values);

        assert_eq!(stats.mean, 3.0);
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 5.0);
        assert!(stats.stddev > 0.0);

        assert!(stats.min <= stats.quantiles.p25);
        assert!(stats.quantiles.p25 <= stats.quantiles.p50);
        assert!(stats.quantiles.p75 <= stats.max);

        let empty_stats = compute_stats_for_flatten(&[]);
        assert_eq!(empty_stats.min, 0.0);
        assert_eq!(empty_stats.max, 0.0);
        assert_eq!(empty_stats.mean, 0.0);
    }
}
