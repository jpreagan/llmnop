use crate::benchmark::BenchmarkResult;
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
pub struct BenchmarkSummary {
    pub version: String,
    pub name: String,
    pub model: String,
    pub mean_input_tokens: u32,
    pub stddev_input_tokens: u32,
    pub mean_output_tokens: u32,
    pub stddev_output_tokens: u32,
    pub num_concurrent_requests: u32,
    pub results: MetricResults,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricResults {
    #[serde(rename = "inter_token_latency_s")]
    pub inter_token_latency_s: MetricStats,
    #[serde(rename = "ttft_s")]
    pub ttft_s: MetricStats,
    #[serde(rename = "end_to_end_latency_s")]
    pub end_to_end_latency_s: MetricStats,
    #[serde(rename = "request_output_throughput_token_per_s")]
    pub request_output_throughput_token_per_s: MetricStats,
    #[serde(rename = "number_input_tokens")]
    pub number_input_tokens: TokenStats,
    #[serde(rename = "number_output_tokens")]
    pub number_output_tokens: TokenStats,
    #[serde(rename = "num_requests_started")]
    pub num_requests_started: usize,
    #[serde(rename = "error_rate")]
    pub error_rate: f64,
    #[serde(rename = "number_errors")]
    pub number_errors: usize,
    #[serde(rename = "error_code_frequency")]
    pub error_code_frequency: String,
    #[serde(rename = "mean_output_throughput_token_per_s")]
    pub mean_output_throughput_token_per_s: f64,
    #[serde(rename = "num_completed_requests")]
    pub num_completed_requests: usize,
    #[serde(rename = "num_completed_requests_per_min")]
    pub num_completed_requests_per_min: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricStats {
    #[serde(rename = "quantiles")]
    pub quantiles: Quantiles,
    pub mean: f64,
    pub min: f64,
    pub max: f64,
    pub stddev: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenStats {
    #[serde(rename = "quantiles")]
    pub quantiles: Quantiles,
    pub mean: f64,
    pub min: String,
    pub max: String,
    pub stddev: f64,
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
                total_output_tokens += br.output_tokens as u64;

                successful_results.push(br.clone());
                let rec = IndividualResponse {
                    error_code: None,
                    error_msg: "".to_string(),
                    inter_token_latency_s: Some(br.inter_token_latency_s),
                    ttft_s: Some(br.ttft.as_secs_f64()),
                    end_to_end_latency_s: Some(br.total_latency.as_secs_f64()),
                    request_output_throughput_token_per_s: Some(br.throughput),
                    number_total_tokens: Some(br.total_tokens),
                    number_output_tokens: Some(br.output_tokens),
                    number_input_tokens: Some(br.input_tokens),
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
            sanitize_filename::sanitize(model.replace('/', "-").replace('.', "-")),
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
            sanitize_filename::sanitize(model.replace('/', "-").replace('.', "-")),
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
) -> BenchmarkSummary {
    use std::time::{SystemTime, UNIX_EPOCH};

    let total_time_s = end_time.duration_since(start_time).as_secs_f64();

    let mut inter_token_vec = Vec::new();
    let mut ttft_vec = Vec::new();
    let mut e2e_vec = Vec::new();
    let mut throughput_vec = Vec::new();
    let mut in_tokens_vec = Vec::new();
    let mut out_tokens_vec = Vec::new();

    for br in successful_results {
        inter_token_vec.push(br.inter_token_latency_s);
        ttft_vec.push(br.ttft.as_secs_f64());
        e2e_vec.push(br.total_latency.as_secs_f64());
        throughput_vec.push(br.throughput);
        in_tokens_vec.push(br.input_tokens as f64);
        out_tokens_vec.push(br.output_tokens as f64);
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

    BenchmarkSummary {
        version: "2023-08-31".to_string(),
        name: format!(
            "{}_{}_{}_summary",
            sanitize_filename::sanitize(model.replace('/', "-").replace('.', "-")),
            mean_input_tokens,
            mean_output_tokens
        ),
        model: model.to_string(),
        mean_input_tokens,
        stddev_input_tokens,
        mean_output_tokens,
        stddev_output_tokens,
        num_concurrent_requests,
        results: MetricResults {
            inter_token_latency_s: MetricStats {
                quantiles: inter_stats.quantiles,
                mean: inter_stats.mean,
                min: inter_stats.min,
                max: inter_stats.max,
                stddev: inter_stats.stddev,
            },
            ttft_s: MetricStats {
                quantiles: ttft_stats.quantiles,
                mean: ttft_stats.mean,
                min: ttft_stats.min,
                max: ttft_stats.max,
                stddev: ttft_stats.stddev,
            },
            end_to_end_latency_s: MetricStats {
                quantiles: e2e_stats.quantiles,
                mean: e2e_stats.mean,
                min: e2e_stats.min,
                max: e2e_stats.max,
                stddev: e2e_stats.stddev,
            },
            request_output_throughput_token_per_s: MetricStats {
                quantiles: thr_stats.quantiles,
                mean: thr_stats.mean,
                min: thr_stats.min,
                max: thr_stats.max,
                stddev: thr_stats.stddev,
            },
            number_input_tokens: TokenStats {
                quantiles: in_stats.quantiles,
                mean: in_stats.mean,
                min: format!("{}", in_stats.min),
                max: format!("{}", in_stats.max),
                stddev: in_stats.stddev,
            },
            number_output_tokens: TokenStats {
                quantiles: out_stats.quantiles,
                mean: out_stats.mean,
                min: format!("{}", out_stats.min),
                max: format!("{}", out_stats.max),
                stddev: out_stats.stddev,
            },
            num_requests_started,
            error_rate,
            number_errors: num_errors,
            error_code_frequency,
            mean_output_throughput_token_per_s,
            num_completed_requests,
            num_completed_requests_per_min,
        },
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
    fn test_serialization_roundtrip() {
        let quantiles = Quantiles {
            p25: 0.1,
            p50: 0.2,
            p75: 0.3,
            p90: 0.4,
            p95: 0.5,
            p99: 0.6,
        };
        let metric_stats = MetricStats {
            quantiles,
            mean: 0.25,
            min: 0.05,
            max: 0.8,
            stddev: 0.1,
        };

        let json = serde_json::to_string(&metric_stats).unwrap();
        let deserialized: MetricStats = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.quantiles.p25, 0.1);
        assert_eq!(deserialized.mean, 0.25);
    }

    #[test]
    fn test_stats_computation() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let stats = compute_stats_for_flatten(&values);

        assert_eq!(stats.mean, 3.0);
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 5.0);
        assert!(stats.stddev > 0.0);

        // Test ordering invariant
        assert!(stats.min <= stats.quantiles.p25);
        assert!(stats.quantiles.p25 <= stats.quantiles.p50);
        assert!(stats.quantiles.p75 <= stats.max);

        // Test empty case
        let empty_stats = compute_stats_for_flatten(&[]);
        assert_eq!(empty_stats.min, 0.0);
        assert_eq!(empty_stats.max, 0.0);
        assert_eq!(empty_stats.mean, 0.0);
    }
}
