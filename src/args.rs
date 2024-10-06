use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// API key for the inference server
    #[arg(short = 'k', long)]
    pub api_key: String,

    /// Base URL for the inference server
    #[arg(short = 'u', long)]
    pub base_url: String,

    /// Model to benchmark
    #[arg(short, long)]
    pub model: String,

    /// Number of iterations to run
    #[arg(short = 'n', long, default_value = "2")]
    pub num_iterations: u32,

    /// Number of concurrent requests
    #[arg(short = 'c', long, default_value = "1")]
    pub concurrency: u32,

    /// Mean number of tokens to send in the prompt for the request
    #[arg(long, default_value = "550")]
    pub mean_input_tokens: u32,

    /// Standard deviation of number of tokens to send in the prompt for the request
    #[arg(long, default_value = "150")]
    pub stddev_input_tokens: u32,

    /// Mean number of tokens to generate from each LLM request
    #[arg(long, default_value = "150")]
    pub mean_output_tokens: u32,

    /// Standard deviation on the number of tokens to generate per LLM request
    #[arg(long, default_value = "10")]
    pub stddev_output_tokens: u32,
}
