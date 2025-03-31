use crate::metrics::Metrics;

pub fn display_results(metrics: &Metrics) {
    println!("Benchmark Results:");
    println!("------------------");
    println!("Time to First Token (TTFT): {:?}", metrics.ttft);
    println!("Total Latency: {:?}", metrics.total_latency);
    println!("Throughput: {:.2} tokens/second", metrics.throughput);
    println!("Input Tokens: {}", metrics.input_tokens);
    println!("Output Tokens: {}", metrics.output_tokens);
}
