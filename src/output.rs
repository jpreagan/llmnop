use crate::metrics::Metrics;

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
