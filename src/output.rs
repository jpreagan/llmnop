use crate::benchmark::BenchmarkResult;
use crate::metrics::Metrics;
use serde::{Deserialize, Serialize};
use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndividualResponse {
    #[serde(rename = "error_code")]
    pub error_code: Option<i32>,

    #[serde(rename = "error_msg")]
    pub error_msg: String,

    #[serde(rename = "inter_token_latency_s")]
    pub inter_token_latency_s: Option<f64>,

    #[serde(rename = "ttft_s")]
    pub ttft_s: Option<f64>,

    #[serde(rename = "end_to_end_latency_s")]
    pub end_to_end_latency_s: Option<f64>,

    #[serde(rename = "request_output_throughput_token_per_s")]
    pub request_output_throughput_token_per_s: Option<f64>,

    #[serde(rename = "number_total_tokens")]
    pub number_total_tokens: Option<u32>,

    #[serde(rename = "number_output_tokens")]
    pub number_output_tokens: Option<u32>,

    #[serde(rename = "number_input_tokens")]
    pub number_input_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlattenedSummary {
    pub version: String,
    pub name: String,
    pub model: String,
    pub mean_input_tokens: u32,
    pub stddev_input_tokens: u32,
    pub mean_output_tokens: u32,
    pub stddev_output_tokens: u32,
    pub num_concurrent_requests: u32,

    #[serde(rename = "results_inter_token_latency_s_quantiles_p25")]
    pub inter_token_latency_s_q25: f64,
    #[serde(rename = "results_inter_token_latency_s_quantiles_p50")]
    pub inter_token_latency_s_q50: f64,
    #[serde(rename = "results_inter_token_latency_s_quantiles_p75")]
    pub inter_token_latency_s_q75: f64,
    #[serde(rename = "results_inter_token_latency_s_quantiles_p90")]
    pub inter_token_latency_s_q90: f64,
    #[serde(rename = "results_inter_token_latency_s_quantiles_p95")]
    pub inter_token_latency_s_q95: f64,
    #[serde(rename = "results_inter_token_latency_s_quantiles_p99")]
    pub inter_token_latency_s_q99: f64,
    #[serde(rename = "results_inter_token_latency_s_mean")]
    pub inter_token_latency_s_mean: f64,
    #[serde(rename = "results_inter_token_latency_s_min")]
    pub inter_token_latency_s_min: f64,
    #[serde(rename = "results_inter_token_latency_s_max")]
    pub inter_token_latency_s_max: f64,
    #[serde(rename = "results_inter_token_latency_s_stddev")]
    pub inter_token_latency_s_stddev: f64,

    #[serde(rename = "results_ttft_s_quantiles_p25")]
    pub ttft_s_q25: f64,
    #[serde(rename = "results_ttft_s_quantiles_p50")]
    pub ttft_s_q50: f64,
    #[serde(rename = "results_ttft_s_quantiles_p75")]
    pub ttft_s_q75: f64,
    #[serde(rename = "results_ttft_s_quantiles_p90")]
    pub ttft_s_q90: f64,
    #[serde(rename = "results_ttft_s_quantiles_p95")]
    pub ttft_s_q95: f64,
    #[serde(rename = "results_ttft_s_quantiles_p99")]
    pub ttft_s_q99: f64,
    #[serde(rename = "results_ttft_s_mean")]
    pub ttft_s_mean: f64,
    #[serde(rename = "results_ttft_s_min")]
    pub ttft_s_min: f64,
    #[serde(rename = "results_ttft_s_max")]
    pub ttft_s_max: f64,
    #[serde(rename = "results_ttft_s_stddev")]
    pub ttft_s_stddev: f64,

    #[serde(rename = "results_end_to_end_latency_s_quantiles_p25")]
    pub e2e_latency_s_q25: f64,
    #[serde(rename = "results_end_to_end_latency_s_quantiles_p50")]
    pub e2e_latency_s_q50: f64,
    #[serde(rename = "results_end_to_end_latency_s_quantiles_p75")]
    pub e2e_latency_s_q75: f64,
    #[serde(rename = "results_end_to_end_latency_s_quantiles_p90")]
    pub e2e_latency_s_q90: f64,
    #[serde(rename = "results_end_to_end_latency_s_quantiles_p95")]
    pub e2e_latency_s_q95: f64,
    #[serde(rename = "results_end_to_end_latency_s_quantiles_p99")]
    pub e2e_latency_s_q99: f64,
    #[serde(rename = "results_end_to_end_latency_s_mean")]
    pub e2e_latency_s_mean: f64,
    #[serde(rename = "results_end_to_end_latency_s_min")]
    pub e2e_latency_s_min: f64,
    #[serde(rename = "results_end_to_end_latency_s_max")]
    pub e2e_latency_s_max: f64,
    #[serde(rename = "results_end_to_end_latency_s_stddev")]
    pub e2e_latency_s_stddev: f64,

    #[serde(rename = "results_request_output_throughput_token_per_s_quantiles_p25")]
    pub throughput_s_q25: f64,
    #[serde(rename = "results_request_output_throughput_token_per_s_quantiles_p50")]
    pub throughput_s_q50: f64,
    #[serde(rename = "results_request_output_throughput_token_per_s_quantiles_p75")]
    pub throughput_s_q75: f64,
    #[serde(rename = "results_request_output_throughput_token_per_s_quantiles_p90")]
    pub throughput_s_q90: f64,
    #[serde(rename = "results_request_output_throughput_token_per_s_quantiles_p95")]
    pub throughput_s_q95: f64,
    #[serde(rename = "results_request_output_throughput_token_per_s_quantiles_p99")]
    pub throughput_s_q99: f64,
    #[serde(rename = "results_request_output_throughput_token_per_s_mean")]
    pub throughput_s_mean: f64,
    #[serde(rename = "results_request_output_throughput_token_per_s_min")]
    pub throughput_s_min: f64,
    #[serde(rename = "results_request_output_throughput_token_per_s_max")]
    pub throughput_s_max: f64,
    #[serde(rename = "results_request_output_throughput_token_per_s_stddev")]
    pub throughput_s_stddev: f64,

    #[serde(rename = "results_number_input_tokens_quantiles_p25")]
    pub num_input_tokens_q25: f64,
    #[serde(rename = "results_number_input_tokens_quantiles_p50")]
    pub num_input_tokens_q50: f64,
    #[serde(rename = "results_number_input_tokens_quantiles_p75")]
    pub num_input_tokens_q75: f64,
    #[serde(rename = "results_number_input_tokens_quantiles_p90")]
    pub num_input_tokens_q90: f64,
    #[serde(rename = "results_number_input_tokens_quantiles_p95")]
    pub num_input_tokens_q95: f64,
    #[serde(rename = "results_number_input_tokens_quantiles_p99")]
    pub num_input_tokens_q99: f64,
    #[serde(rename = "results_number_input_tokens_mean")]
    pub num_input_tokens_mean: f64,
    #[serde(rename = "results_number_input_tokens_min")]
    pub num_input_tokens_min: String,
    #[serde(rename = "results_number_input_tokens_max")]
    pub num_input_tokens_max: String,
    #[serde(rename = "results_number_input_tokens_stddev")]
    pub num_input_tokens_stddev: f64,

    #[serde(rename = "results_number_output_tokens_quantiles_p25")]
    pub num_output_tokens_q25: f64,
    #[serde(rename = "results_number_output_tokens_quantiles_p50")]
    pub num_output_tokens_q50: f64,
    #[serde(rename = "results_number_output_tokens_quantiles_p75")]
    pub num_output_tokens_q75: f64,
    #[serde(rename = "results_number_output_tokens_quantiles_p90")]
    pub num_output_tokens_q90: f64,
    #[serde(rename = "results_number_output_tokens_quantiles_p95")]
    pub num_output_tokens_q95: f64,
    #[serde(rename = "results_number_output_tokens_quantiles_p99")]
    pub num_output_tokens_q99: f64,
    #[serde(rename = "results_number_output_tokens_mean")]
    pub num_output_tokens_mean: f64,
    #[serde(rename = "results_number_output_tokens_min")]
    pub num_output_tokens_min: String,
    #[serde(rename = "results_number_output_tokens_max")]
    pub num_output_tokens_max: String,
    #[serde(rename = "results_number_output_tokens_stddev")]
    pub num_output_tokens_stddev: f64,

    #[serde(rename = "results_num_requests_started")]
    pub num_requests_started: usize,
    #[serde(rename = "results_error_rate")]
    pub error_rate: f64,
    #[serde(rename = "results_number_errors")]
    pub number_errors: usize,
    #[serde(rename = "results_error_code_frequency")]
    pub error_code_frequency: String,
    #[serde(rename = "results_mean_output_throughput_token_per_s")]
    pub mean_output_throughput_token_per_s: f64,
    #[serde(rename = "results_num_completed_requests")]
    pub num_completed_requests: usize,
    #[serde(rename = "results_num_completed_requests_per_min")]
    pub num_completed_requests_per_min: f64,

    #[serde(rename = "timestamp")]
    pub timestamp: u64,
}

pub fn write_results_json(
    results_dir: &str,
    model: &str,
    mean_input_tokens: u32,
    stddev_input_tokens: u32,
    mean_output_tokens: u32,
    stddev_output_tokens: u32,
    num_concurrent_requests: u32,
    all_results: &[Result<BenchmarkResult, String>],
    total_start_time: std::time::Instant,
    total_end_time: std::time::Instant,
) -> std::io::Result<()> {
    if results_dir.is_empty() {
        return Ok(());
    }
    create_dir_all(results_dir)?;

    let mut individual_responses = Vec::with_capacity(all_results.len());
    let mut total_output_tokens = 0_u64;
    let mut successful_results = Vec::new();

    for result in all_results {
        match result {
            Ok(br) => {
                let metrics: Metrics = br.clone().into();
                total_output_tokens += metrics.number_output_tokens as u64;

                successful_results.push(br.clone());
                let rec = IndividualResponse {
                    error_code: None,
                    error_msg: "".to_string(),
                    inter_token_latency_s: Some(metrics.inter_token_latency_s),
                    ttft_s: Some(metrics.ttft_s),
                    end_to_end_latency_s: Some(metrics.end_to_end_latency_s),
                    request_output_throughput_token_per_s: Some(
                        metrics.request_output_throughput_token_per_s,
                    ),
                    number_total_tokens: Some(metrics.number_total_tokens),
                    number_output_tokens: Some(metrics.number_output_tokens),
                    number_input_tokens: Some(metrics.number_input_tokens),
                };
                individual_responses.push(rec);
            }
            Err(msg) => {
                let rec = IndividualResponse {
                    error_code: Some(1),
                    error_msg: msg.clone(),
                    inter_token_latency_s: None,
                    ttft_s: None,
                    end_to_end_latency_s: None,
                    request_output_throughput_token_per_s: None,
                    number_total_tokens: None,
                    number_output_tokens: None,
                    number_input_tokens: None,
                };
                individual_responses.push(rec);
            }
        }
    }

    {
        let file_name = format!(
            "{}_{}_{}_individual_responses.json",
            sanitize_filename::sanitize(model),
            mean_input_tokens,
            mean_output_tokens
        );

        let path = Path::new(results_dir).join(file_name);
        let mut f = File::create(&path)?;
        let resp_json = serde_json::to_string_pretty(&individual_responses)?;
        f.write_all(resp_json.as_bytes())?;
    }

    {
        let summary_filename = format!(
            "{}_{}_{}_summary.json",
            sanitize_filename::sanitize(model),
            mean_input_tokens,
            mean_output_tokens
        );
        let summary_path = Path::new(results_dir).join(summary_filename);

        let flattened = build_flattened_summary(
            model,
            mean_input_tokens,
            stddev_input_tokens,
            mean_output_tokens,
            stddev_output_tokens,
            num_concurrent_requests,
            &successful_results,
            all_results.len(),
            all_results.iter().filter(|r| r.is_err()).count(),
            total_output_tokens,
            total_start_time,
            total_end_time,
        );

        let mut sf = File::create(&summary_path)?;
        let summary_json = serde_json::to_string_pretty(&flattened)?;
        sf.write_all(summary_json.as_bytes())?;
    }

    Ok(())
}

fn build_flattened_summary(
    model: &str,
    mean_input_tokens: u32,
    stddev_input_tokens: u32,
    mean_output_tokens: u32,
    stddev_output_tokens: u32,
    num_concurrent_requests: u32,
    successful_results: &[BenchmarkResult],
    num_requests_started: usize,
    num_errors: usize,
    total_output_tokens: u64,
    start_time: std::time::Instant,
    end_time: std::time::Instant,
) -> FlattenedSummary {
    use crate::metrics::Metrics;
    use std::time::{SystemTime, UNIX_EPOCH};

    let total_time_s = end_time.duration_since(start_time).as_secs_f64();

    let mut inter_token_vec = Vec::new();
    let mut ttft_vec = Vec::new();
    let mut e2e_vec = Vec::new();
    let mut throughput_vec = Vec::new();
    let mut in_tokens_vec = Vec::new();
    let mut out_tokens_vec = Vec::new();

    for br in successful_results {
        let m: Metrics = br.clone().into();
        inter_token_vec.push(m.inter_token_latency_s);
        ttft_vec.push(m.ttft_s);
        e2e_vec.push(m.end_to_end_latency_s);
        throughput_vec.push(m.request_output_throughput_token_per_s);
        in_tokens_vec.push(m.number_input_tokens as f64);
        out_tokens_vec.push(m.number_output_tokens as f64);
    }

    let inter_stats = compute_stats_for_flatten(&inter_token_vec);
    let ttft_stats = compute_stats_for_flatten(&ttft_vec);
    let e2e_stats = compute_stats_for_flatten(&e2e_vec);
    let thr_stats = compute_stats_for_flatten(&throughput_vec);
    let in_stats = compute_stats_for_flatten(&in_tokens_vec);
    let out_stats = compute_stats_for_flatten(&out_tokens_vec);

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

    let mean_output_throughput_token_per_s = if total_time_s > 0.0 {
        total_output_tokens as f64 / total_time_s
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

    FlattenedSummary {
        version: "2023-08-31".to_string(),
        name: format!(
            "{}_{}_{}_summary",
            sanitize_filename(model),
            mean_input_tokens,
            mean_output_tokens
        ),
        model: model.to_string(),
        mean_input_tokens,
        stddev_input_tokens,
        mean_output_tokens,
        stddev_output_tokens,
        num_concurrent_requests,

        inter_token_latency_s_q25: inter_stats.q25,
        inter_token_latency_s_q50: inter_stats.q50,
        inter_token_latency_s_q75: inter_stats.q75,
        inter_token_latency_s_q90: inter_stats.q90,
        inter_token_latency_s_q95: inter_stats.q95,
        inter_token_latency_s_q99: inter_stats.q99,
        inter_token_latency_s_mean: inter_stats.mean,
        inter_token_latency_s_min: inter_stats.min,
        inter_token_latency_s_max: inter_stats.max,
        inter_token_latency_s_stddev: inter_stats.stddev,

        ttft_s_q25: ttft_stats.q25,
        ttft_s_q50: ttft_stats.q50,
        ttft_s_q75: ttft_stats.q75,
        ttft_s_q90: ttft_stats.q90,
        ttft_s_q95: ttft_stats.q95,
        ttft_s_q99: ttft_stats.q99,
        ttft_s_mean: ttft_stats.mean,
        ttft_s_min: ttft_stats.min,
        ttft_s_max: ttft_stats.max,
        ttft_s_stddev: ttft_stats.stddev,

        e2e_latency_s_q25: e2e_stats.q25,
        e2e_latency_s_q50: e2e_stats.q50,
        e2e_latency_s_q75: e2e_stats.q75,
        e2e_latency_s_q90: e2e_stats.q90,
        e2e_latency_s_q95: e2e_stats.q95,
        e2e_latency_s_q99: e2e_stats.q99,
        e2e_latency_s_mean: e2e_stats.mean,
        e2e_latency_s_min: e2e_stats.min,
        e2e_latency_s_max: e2e_stats.max,
        e2e_latency_s_stddev: e2e_stats.stddev,

        throughput_s_q25: thr_stats.q25,
        throughput_s_q50: thr_stats.q50,
        throughput_s_q75: thr_stats.q75,
        throughput_s_q90: thr_stats.q90,
        throughput_s_q95: thr_stats.q95,
        throughput_s_q99: thr_stats.q99,
        throughput_s_mean: thr_stats.mean,
        throughput_s_min: thr_stats.min,
        throughput_s_max: thr_stats.max,
        throughput_s_stddev: thr_stats.stddev,

        num_input_tokens_q25: in_stats.q25,
        num_input_tokens_q50: in_stats.q50,
        num_input_tokens_q75: in_stats.q75,
        num_input_tokens_q90: in_stats.q90,
        num_input_tokens_q95: in_stats.q95,
        num_input_tokens_q99: in_stats.q99,
        num_input_tokens_mean: in_stats.mean,
        num_input_tokens_min: format!("{}", in_stats.min),
        num_input_tokens_max: format!("{}", in_stats.max),
        num_input_tokens_stddev: in_stats.stddev,

        num_output_tokens_q25: out_stats.q25,
        num_output_tokens_q50: out_stats.q50,
        num_output_tokens_q75: out_stats.q75,
        num_output_tokens_q90: out_stats.q90,
        num_output_tokens_q95: out_stats.q95,
        num_output_tokens_q99: out_stats.q99,
        num_output_tokens_mean: out_stats.mean,
        num_output_tokens_min: format!("{}", out_stats.min),
        num_output_tokens_max: format!("{}", out_stats.max),
        num_output_tokens_stddev: out_stats.stddev,

        num_requests_started,
        error_rate,
        number_errors: num_errors,
        error_code_frequency,
        mean_output_throughput_token_per_s,
        num_completed_requests,
        num_completed_requests_per_min,
        timestamp,
    }
}

#[derive(Default)]
struct StatSet {
    q25: f64,
    q50: f64,
    q75: f64,
    q90: f64,
    q95: f64,
    q99: f64,
    mean: f64,
    min: f64,
    max: f64,
    stddev: f64,
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
    let q25 = percentile(&sorted, 0.25);
    let q50 = percentile(&sorted, 0.50);
    let q75 = percentile(&sorted, 0.75);
    let q90 = percentile(&sorted, 0.90);
    let q95 = percentile(&sorted, 0.95);
    let q99 = percentile(&sorted, 0.99);

    StatSet {
        q25,
        q50,
        q75,
        q90,
        q95,
        q99,
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

fn sanitize_filename(model: &str) -> String {
    let re = regex::Regex::new(r"[^\w\d-]+").unwrap();
    let re_dupes = regex::Regex::new(r"-{2,}").unwrap();
    let tmp = re.replace_all(model, "-");
    re_dupes.replace_all(&tmp, "-").to_string()
}
