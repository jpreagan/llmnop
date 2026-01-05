use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long, help = "Model name")]
    pub model: String,

    #[arg(long, help = "Hugging Face tokenizer (defaults to model name)")]
    pub tokenizer: Option<String>,

    #[arg(long, default_value = "2", help = "Number of requests")]
    pub max_num_completed_requests: u32,

    #[arg(long, default_value = "1", help = "Parallel requests")]
    pub num_concurrent_requests: u32,

    #[arg(long, default_value = "550", help = "Target input length")]
    pub mean_input_tokens: u32,

    #[arg(long, default_value = "150", help = "Input length variance")]
    pub stddev_input_tokens: u32,

    #[arg(long, help = "Target output length [default: none]")]
    pub mean_output_tokens: Option<u32>,

    #[arg(long, default_value = "10", help = "Output length variance")]
    pub stddev_output_tokens: u32,

    #[arg(long, default_value = "result_outputs", help = "Output directory")]
    pub results_dir: String,

    #[arg(long, default_value = "600", help = "Request timeout")]
    pub timeout: u64,

    #[arg(long, help = "Hide progress bar")]
    pub no_progress: bool,
}
