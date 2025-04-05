use crate::benchmark::BenchmarkResult;
use crate::metrics::Metrics;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

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
pub struct MetricStats {
    pub quantiles: HashMap<String, f64>,
    pub mean: f64,
    pub min: f64,
    pub max: f64,
    pub stddev: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub version: String,
    pub name: String,
    pub model: String,
    pub mean_input_tokens: u32,
    pub stddev_input_tokens: u32,
    pub mean_output_tokens: u32,
    pub stddev_output_tokens: u32,
    pub num_concurrent_requests: u32,

    #[serde(rename = "results_inter_token_latency_s")]
    pub inter_token_latency: MetricStats,

    #[serde(rename = "results_ttft_s")]
    pub ttft_s: MetricStats,

    #[serde(rename = "results_end_to_end_latency_s")]
    pub e2e_latency: MetricStats,

    #[serde(rename = "results_request_output_throughput_token_per_s")]
    pub per_request_throughput: MetricStats,

    #[serde(rename = "results_number_input_tokens")]
    pub num_input_tokens: MetricStats,

    #[serde(rename = "results_number_output_tokens")]
    pub num_output_tokens: MetricStats,

    #[serde(rename = "results_num_requests_started")]
    pub num_req_started: usize,

    #[serde(rename = "results_error_rate")]
    pub error_rate: f64,

    #[serde(rename = "results_number_errors")]
    pub num_errors: usize,

    #[serde(rename = "results_error_code_frequency")]
    pub error_code_freq: String,

    #[serde(rename = "results_mean_output_throughput_token_per_s")]
    pub overall_output_throughput: f64,

    #[serde(rename = "results_num_completed_requests")]
    pub num_completed_requests: usize,

    #[serde(rename = "results_num_completed_requests_per_min")]
    pub completed_requests_per_min: f64,

    #[serde(rename = "timestamp")]
    pub timestamp: u64,
}

pub fn display_results(metrics: &Metrics) {
    println!("\nRequest Performance Metrics:");
    println!("---------------------------");
    println!(
        "inter_token_latency_s: {:.6}",
        metrics.inter_token_latency_s
    );
    println!("ttft_s: {:.6}", metrics.ttft_s);
    println!("end_to_end_latency_s: {:.6}", metrics.end_to_end_latency_s);
    println!(
        "request_output_throughput_token_per_s: {:.6}",
        metrics.request_output_throughput_token_per_s
    );
    println!("number_input_tokens: {}", metrics.number_input_tokens);
    println!("number_output_tokens: {}", metrics.number_output_tokens);
    println!("number_total_tokens: {}", metrics.number_total_tokens);
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
    let mut total_output_tokens = 0u64;
    let mut successful_results = Vec::new();

    for result in all_results {
        match result {
            Ok(br) => {
                let metrics: Metrics = br.clone().into();
                total_output_tokens += metrics.number_output_tokens as u64;
                successful_results.push(br.clone());
                individual_responses.push(IndividualResponse {
                    error_code: None,
                    error_msg: "".to_string(),
                    inter_token_latency_s: Some(metrics.inter_token_latency_s),
                    ttft_s: Some(metrics.ttft_s),
                    end_to_end_latency_s: Some(metrics.end_to_end_latency_s),
                    request_output_throughput_token_per_s: Some(
                        metrics.request_output_throughput_token_per_s,
                    ),
                    number_total_tokens: Some(
                        metrics.number_input_tokens + metrics.number_output_tokens,
                    ),
                    number_output_tokens: Some(metrics.number_output_tokens),
                    number_input_tokens: Some(metrics.number_input_tokens),
                });
            }
            Err(e) => {
                individual_responses.push(IndividualResponse {
                    error_code: Some(1),
                    error_msg: e.clone(),
                    inter_token_latency_s: None,
                    ttft_s: None,
                    end_to_end_latency_s: None,
                    request_output_throughput_token_per_s: None,
                    number_total_tokens: None,
                    number_output_tokens: None,
                    number_input_tokens: None,
                });
            }
        }
    }

    let sanitized_name = sanitize_filename(model);
    let individual_filename = format!(
        "{}_{}_{}_individual_responses.json",
        sanitized_name, mean_input_tokens, mean_output_tokens
    );
    let individual_path = Path::new(results_dir).join(&individual_filename);
    let mut f = File::create(&individual_path)?;
    let indiv_json = serde_json::to_string_pretty(&individual_responses)?;
    f.write_all(indiv_json.as_bytes())?;

    let summary_filename = format!(
        "{}_{}_{}_summary.json",
        sanitized_name, mean_input_tokens, mean_output_tokens
    );
    let summary_path = Path::new(results_dir).join(summary_filename);
    let summary = compute_summary(
        model,
        mean_input_tokens,
        stddev_input_tokens,
        mean_output_tokens,
        stddev_output_tokens,
        num_concurrent_requests,
        &successful_results,
        total_start_time,
        total_end_time,
        total_output_tokens,
        all_results.len(),
        all_results.iter().filter(|r| r.is_err()).count(),
    );
    let mut sf = File::create(&summary_path)?;
    let summary_json = serde_json::to_string_pretty(&summary)?;
    sf.write_all(summary_json.as_bytes())?;

    Ok(())
}

fn compute_summary(
    model: &str,
    mean_input_tokens: u32,
    stddev_input_tokens: u32,
    mean_output_tokens: u32,
    stddev_output_tokens: u32,
    num_concurrent_requests: u32,
    successful_results: &[BenchmarkResult],
    total_start_time: std::time::Instant,
    total_end_time: std::time::Instant,
    total_output_tokens: u64,
    num_requests_started: usize,
    num_errors: usize,
) -> Summary {
    let total_time_s = total_end_time
        .duration_since(total_start_time)
        .as_secs_f64();
    let n = successful_results.len();
    let (mut inter_token_vec, mut ttft_vec, mut e2e_vec, mut throughput_vec) = (
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
    );
    let (mut in_tokens_vec, mut out_tokens_vec) = (Vec::with_capacity(n), Vec::with_capacity(n));

    for br in successful_results {
        let m: Metrics = br.clone().into();
        inter_token_vec.push(m.inter_token_latency_s);
        ttft_vec.push(m.ttft_s);
        e2e_vec.push(m.end_to_end_latency_s);
        throughput_vec.push(m.request_output_throughput_token_per_s);
        in_tokens_vec.push(m.number_input_tokens as f64);
        out_tokens_vec.push(m.number_output_tokens as f64);
    }

    let inter_token_stats = compute_metric_stats(&inter_token_vec);
    let ttft_stats = compute_metric_stats(&ttft_vec);
    let e2e_stats = compute_metric_stats(&e2e_vec);
    let throughput_stats = compute_metric_stats(&throughput_vec);
    let in_token_stats = compute_metric_stats(&in_tokens_vec);
    let out_token_stats = compute_metric_stats(&out_tokens_vec);

    let error_rate = if num_requests_started > 0 {
        num_errors as f64 / num_requests_started as f64
    } else {
        0.0
    };
    let error_code_freq = if num_errors > 0 {
        "{\"1\": ".to_string() + &num_errors.to_string() + "}"
    } else {
        "{}".to_string()
    };
    let overall_output_throughput = if total_time_s > 0.0 {
        total_output_tokens as f64 / total_time_s
    } else {
        0.0
    };
    let completed_requests_per_min = if total_time_s > 0.0 {
        (n as f64) / total_time_s * 60.0
    } else {
        0.0
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Summary {
        version: "2023-08-31".into(),
        name: format!(
            "{}_{}_{}_summary",
            sanitize_filename(model),
            mean_input_tokens,
            mean_output_tokens
        ),
        model: model.into(),
        mean_input_tokens,
        stddev_input_tokens,
        mean_output_tokens,
        stddev_output_tokens,
        num_concurrent_requests,

        inter_token_latency: inter_token_stats,
        ttft_s: ttft_stats,
        e2e_latency: e2e_stats,
        per_request_throughput: throughput_stats,
        num_input_tokens: in_token_stats,
        num_output_tokens: out_token_stats,

        num_req_started: num_requests_started,
        error_rate,
        num_errors,
        error_code_freq,
        overall_output_throughput,
        num_completed_requests: n,
        completed_requests_per_min,
        timestamp: now,
    }
}

fn compute_metric_stats(values: &[f64]) -> MetricStats {
    use std::f64;

    if values.is_empty() {
        return MetricStats {
            quantiles: HashMap::new(),
            mean: 0.0,
            min: 0.0,
            max: 0.0,
            stddev: 0.0,
        };
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mean = sorted.iter().copied().sum::<f64>() / sorted.len() as f64;
    let min = sorted[0];
    let max = sorted[sorted.len() - 1];
    let stddev = if sorted.len() > 1 {
        let var = sorted.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>()
            / (sorted.len() as f64 - 1.0);
        var.sqrt()
    } else {
        0.0
    };

    let quantiles = [0.25, 0.50, 0.75, 0.90, 0.95, 0.99];
    let mut qmap = HashMap::new();
    for q in quantiles {
        let idx = ((sorted.len() - 1) as f64 * q).floor() as usize;
        let val = sorted[idx];
        let key = format!("p{}", (q * 100.0) as i32);
        qmap.insert(key, val);
    }
    MetricStats {
        quantiles: qmap,
        mean,
        min,
        max,
        stddev,
    }
}

fn sanitize_filename(model: &str) -> String {
    let re = regex::Regex::new(r"[^\w\d-]+").unwrap();
    let re_dupes = regex::Regex::new(r"-{2,}").unwrap();

    let tmp = re.replace_all(model, "-");
    re_dupes.replace_all(&tmp, "-").to_string()
}
