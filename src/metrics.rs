use std::time::Duration;

pub struct Metrics {
    pub ttft: Duration,
    pub total_latency: Duration,
    pub throughput: f64,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

impl From<crate::benchmark::BenchmarkResult> for Metrics {
    fn from(result: crate::benchmark::BenchmarkResult) -> Self {
        Metrics {
            ttft: result.ttft,
            total_latency: result.total_latency,
            throughput: result.throughput,
            input_tokens: result.input_tokens,
            output_tokens: result.output_tokens,
        }
    }
}
