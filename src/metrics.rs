use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct Metrics {
    pub ttft: Duration,
    pub total_latency: Duration,
    pub throughput: f64,
    pub input_tokens: u32,
    pub output_tokens: u32,
}
