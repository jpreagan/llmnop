use crate::metrics::Metrics;

/// Displays the benchmark results.
///
/// # Arguments
///
/// * `metrics` - A reference to a `Metrics` struct containing the benchmark results.
///
/// # Returns
///
/// This function doesn't return a value; it prints the results to stdout.
pub fn display_results(metrics: &Metrics) {
    println!("Benchmark Results:");
    println!("------------------");
    println!("Time to First Token (TTFT): {:?}", metrics.ttft);
    println!("Total Latency: {:?}", metrics.total_latency);
    println!("Throughput: {:.2} tokens/second", metrics.throughput);
    println!("Input Tokens: {}", metrics.input_tokens);
    println!("Output Tokens: {}", metrics.output_tokens);
}
