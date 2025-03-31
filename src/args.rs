use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    pub model: String,

    #[arg(short = 'n', long, default_value = "2")]
    pub num_iterations: u32,

    #[arg(short = 'c', long, default_value = "1")]
    pub concurrency: u32,

    #[arg(long, default_value = "550")]
    pub mean_input_tokens: u32,

    #[arg(long, default_value = "150")]
    pub stddev_input_tokens: u32,

    #[arg(long, default_value = "150")]
    pub mean_output_tokens: u32,

    #[arg(long, default_value = "10")]
    pub stddev_output_tokens: u32,
}
