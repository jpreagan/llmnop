use crate::benchmark::BenchmarkResult;

#[derive(Debug, Clone)]
pub struct Metrics {
    pub ttft_s: f64,
    pub end_to_end_latency_s: f64,
    pub request_output_throughput_token_per_s: f64,
    pub number_input_tokens: u32,
    pub number_output_tokens: u32,
    pub number_total_tokens: u32,
    pub inter_token_latency_s: f64,
}

impl From<BenchmarkResult> for Metrics {
    fn from(br: BenchmarkResult) -> Self {
        Self {
            ttft_s: br.ttft.as_secs_f64(),
            end_to_end_latency_s: br.total_latency.as_secs_f64(),
            request_output_throughput_token_per_s: br.throughput,
            number_input_tokens: br.input_tokens,
            number_output_tokens: br.output_tokens,
            number_total_tokens: br.input_tokens + br.output_tokens,
            inter_token_latency_s: br.inter_token_latency_s,
        }
    }
}
