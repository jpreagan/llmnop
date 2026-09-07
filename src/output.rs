use crate::args::Args;
use crate::benchmark::{Metrics, Phase, RequestRecord, Status, unix_time_ns};
use anyhow::{Context, Result};
use comfy_table::{Table, presets::UTF8_FULL_CONDENSED};
use directories::ProjectDirs;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Serialize)]
pub struct MetricStats {
    pub count: usize,
    pub mean: Option<f64>,
    pub stddev: Option<f64>,
    pub min: Option<f64>,
    pub p50: Option<f64>,
    pub p90: Option<f64>,
    pub p95: Option<f64>,
    pub p99: Option<f64>,
    pub max: Option<f64>,
}

impl MetricStats {
    fn new(mut values: Vec<f64>) -> Self {
        values.sort_unstable_by(f64::total_cmp);
        let count = values.len();
        let mean = (count > 0).then(|| values.iter().sum::<f64>() / count as f64);
        let stddev = mean.map(|mean| {
            (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / count as f64).sqrt()
        });
        let percentile = |p: f64| {
            if count == 0 {
                return None;
            }
            let index = p * (count - 1) as f64;
            let lo = index.floor() as usize;
            let hi = index.ceil() as usize;
            Some(values[lo] + (values[hi] - values[lo]) * index.fract())
        };
        Self {
            count,
            mean,
            stddev,
            min: values.first().copied(),
            p50: percentile(0.5),
            p90: percentile(0.9),
            p95: percentile(0.95),
            p99: percentile(0.99),
            max: values.last().copied(),
        }
    }
}

#[derive(Debug, Default, Serialize)]
pub struct Counts {
    pub started: usize,
    pub completed: usize,
    pub failed: usize,
    pub timed_out: usize,
    pub cancelled: usize,
    pub completed_at_output_limit: usize,
    pub completed_with_no_text: usize,
}

impl Counts {
    fn from_records(records: &[&RequestRecord]) -> Self {
        let mut counts = Self::default();
        for r in records {
            counts.started += 1;
            match r.status {
                Status::Completed => {
                    counts.completed += 1;
                    counts.completed_at_output_limit += usize::from(matches!(
                        r.finish_reason.as_deref(),
                        Some("length" | "max_tokens" | "max_output_tokens")
                    ));
                    counts.completed_with_no_text += usize::from(r.metrics.delivery_events == 0);
                }
                Status::Failed => counts.failed += 1,
                Status::TimedOut => counts.timed_out += 1,
                Status::Cancelled => counts.cancelled += 1,
            }
        }
        counts
    }

    pub fn unsuccessful(&self) -> usize {
        self.failed + self.timed_out + self.cancelled
    }
}

#[derive(Debug, Default, Serialize)]
pub struct TokenTotals {
    pub input_tokens: u64,
    pub content_tokens: u64,
    pub reasoning_tokens: u64,
    pub generated_tokens: u64,
}

#[derive(Serialize)]
pub struct BenchmarkSummary<'a> {
    pub schema_version: &'static str,
    pub llmnop_version: &'static str,
    pub run_id: String,
    pub configuration: &'a Args,
    pub termination: &'static str,
    pub start_time_unix_ns: Option<u64>,
    pub end_time_unix_ns: Option<u64>,
    pub measurement_duration_ms: Option<f64>,
    pub measurement: Counts,
    pub warmup: Counts,
    pub completion_fraction: Option<f64>,
    pub completed_requests_per_second: Option<f64>,
    pub completed_generated_tokens_per_second: Option<f64>,
    pub metric_population: &'static str,
    pub metrics: BTreeMap<&'static str, MetricStats>,
    pub completed_token_totals: TokenTotals,
    pub errors: BTreeMap<&'static str, usize>,
}

fn metric_values(m: &Metrics) -> [(&'static str, Option<f64>); 13] {
    [
        ("request_latency_ms", m.request_latency_ms),
        ("ttft_ms", m.ttft_ms),
        ("ttfo_ms", m.ttfo_ms),
        ("generation_window_ms", m.generation_window_ms),
        (
            "generation_tokens_per_second",
            m.generation_tokens_per_second,
        ),
        ("mean_inter_token_latency_ms", m.mean_inter_token_latency_ms),
        ("mean_inter_event_latency_ms", m.mean_inter_event_latency_ms),
        ("max_inter_event_latency_ms", m.max_inter_event_latency_ms),
        ("input_tokens", Some(m.input_tokens as f64)),
        ("content_tokens", m.content_tokens.map(|n| n as f64)),
        ("reasoning_tokens", m.reasoning_tokens.map(|n| n as f64)),
        ("generated_tokens", m.generated_tokens.map(|n| n as f64)),
        ("delivery_events", Some(m.delivery_events as f64)),
    ]
}

impl<'a> BenchmarkSummary<'a> {
    pub fn new(
        args: &'a Args,
        run_id: String,
        records: &[RequestRecord],
        interrupted: bool,
    ) -> Self {
        let measured: Vec<_> = records
            .iter()
            .filter(|r| r.phase == Phase::Measurement)
            .collect();
        let warmup: Vec<_> = records
            .iter()
            .filter(|r| r.phase == Phase::Warmup)
            .collect();
        let start = measured.iter().min_by_key(|r| r.start);
        let end = measured.iter().max_by_key(|r| r.end);
        let duration = start
            .zip(end)
            .map(|(a, b)| b.end.duration_since(a.start).as_secs_f64());
        let mut values: BTreeMap<_, Vec<_>> = metric_values(&Metrics::default())
            .into_iter()
            .map(|(key, _)| (key, Vec::new()))
            .collect();
        let mut totals = TokenTotals::default();
        let mut errors = BTreeMap::new();
        for r in &measured {
            if r.status == Status::Completed {
                for (key, value) in metric_values(&r.metrics) {
                    if let Some(value) = value {
                        values.get_mut(key).unwrap().push(value);
                    }
                }
                totals.input_tokens += r.metrics.input_tokens;
                totals.content_tokens += r.metrics.content_tokens.unwrap_or(0);
                totals.reasoning_tokens += r.metrics.reasoning_tokens.unwrap_or(0);
                totals.generated_tokens += r.metrics.generated_tokens.unwrap_or(0);
            }
            if let Some(error) = &r.error {
                *errors.entry(error.category).or_default() += 1;
            }
        }
        let measurement = Counts::from_records(&measured);
        let completion_fraction = (measurement.started > 0)
            .then(|| measurement.completed as f64 / measurement.started as f64);
        let rate_duration = duration.filter(|d| *d > 0.0);
        Self {
            schema_version: "3.0",
            llmnop_version: env!("CARGO_PKG_VERSION"),
            run_id,
            configuration: args,
            termination: if interrupted {
                "interrupted"
            } else {
                "request_count"
            },
            start_time_unix_ns: start.map(|r| r.start_time_unix_ns),
            end_time_unix_ns: end.map(|r| r.end_time_unix_ns),
            measurement_duration_ms: duration.map(|d| d * 1000.0),
            completed_requests_per_second: rate_duration.map(|d| measurement.completed as f64 / d),
            completed_generated_tokens_per_second: rate_duration
                .map(|d| totals.generated_tokens as f64 / d),
            measurement,
            warmup: Counts::from_records(&warmup),
            completion_fraction,
            metric_population: "completed_measurement_requests",
            metrics: values
                .into_iter()
                .map(|(key, values)| (key, MetricStats::new(values)))
                .collect(),
            completed_token_totals: totals,
            errors,
        }
    }

    pub fn table(&self) -> String {
        let mut table = Table::new();
        table.load_style(UTF8_FULL_CONDENSED);
        table.set_header(["Metric", "Samples", "Mean", "Median", "P95", "P99"]);
        for (key, label) in [
            ("request_latency_ms", "Request latency (ms)"),
            ("ttft_ms", "Time to first token (ms)"),
            ("ttfo_ms", "Time to first content (ms)"),
            ("generation_tokens_per_second", "Generation rate (tokens/s)"),
            (
                "mean_inter_token_latency_ms",
                "Estimated inter-token latency (ms)",
            ),
            ("mean_inter_event_latency_ms", "Mean stream-event gap (ms)"),
            (
                "max_inter_event_latency_ms",
                "Longest stream-event gap (ms)",
            ),
            ("input_tokens", "Input tokens"),
            ("content_tokens", "Content tokens"),
            ("reasoning_tokens", "Exposed reasoning tokens"),
            ("generated_tokens", "Generated tokens"),
        ] {
            let m = &self.metrics[key];
            table.add_row(vec![
                label.to_string(),
                m.count.to_string(),
                display(m.mean),
                display(m.p50),
                display(m.p95),
                display(m.p99),
            ]);
        }
        format!(
            "{table}\n\nCompleted: {} / {}  Failed: {}  Timed out: {}  Cancelled: {}\nOutput-limit completions: {}  Empty completions: {}\nDuration: {} ms  Completed requests/s: {}  Generated tokens/s: {}\n",
            self.measurement.completed,
            self.measurement.started,
            self.measurement.failed,
            self.measurement.timed_out,
            self.measurement.cancelled,
            self.measurement.completed_at_output_limit,
            self.measurement.completed_with_no_text,
            display(self.measurement_duration_ms),
            display(self.completed_requests_per_second),
            display(self.completed_generated_tokens_per_second)
        )
    }
}

fn display(value: Option<f64>) -> String {
    value
        .map(|v| format!("{v:.2}"))
        .unwrap_or_else(|| "—".to_string())
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

    pub async fn finish(&mut self, summary: &BenchmarkSummary<'_>) -> Result<()> {
        self.records.flush().await?;
        let path = self.directory.join("summary.json");
        tokio::fs::write(path, serde_json::to_vec_pretty(summary)?).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statistics_use_linear_percentiles_and_population_deviation() {
        let stats = MetricStats::new(vec![5.0, 1.0, 3.0, 2.0, 4.0]);
        assert_eq!(stats.count, 5);
        assert_eq!(stats.mean, Some(3.0));
        assert_eq!(stats.p50, Some(3.0));
        assert_eq!(stats.p95, Some(4.8));
        assert!((stats.stddev.unwrap() - 2f64.sqrt()).abs() < 1e-10);
        let empty = MetricStats::new(vec![]);
        assert_eq!(empty.count, 0);
        assert_eq!(empty.mean, None);
        assert_eq!(empty.p99, None);
    }
}
