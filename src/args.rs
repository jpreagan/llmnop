use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ApiType {
    Chat,
    Responses,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    // Endpoint
    #[arg(
        long,
        help = "Base URL (e.g., http://localhost:8000/v1)",
        help_heading = "Endpoint"
    )]
    pub url: String,

    #[arg(long, help = "API key", help_heading = "Endpoint")]
    pub api_key: Option<String>,

    #[arg(short, long, help = "Model name", help_heading = "Endpoint")]
    pub model: String,

    #[arg(
        long,
        value_enum,
        default_value = "chat",
        help = "API type",
        help_heading = "Endpoint"
    )]
    pub api: ApiType,

    // Request Shaping
    #[arg(
        long,
        default_value = "550",
        help = "Target input length",
        help_heading = "Request Shaping"
    )]
    pub mean_input_tokens: u32,

    #[arg(
        long,
        default_value = "0",
        help = "Input length variance",
        help_heading = "Request Shaping"
    )]
    pub stddev_input_tokens: u32,

    #[arg(
        long,
        help = "Target output length [default: none]",
        help_heading = "Request Shaping"
    )]
    pub mean_output_tokens: Option<u32>,

    #[arg(
        long,
        default_value = "0",
        help = "Output length variance",
        help_heading = "Request Shaping"
    )]
    pub stddev_output_tokens: u32,

    // Load Testing
    #[arg(
        long,
        default_value = "10",
        help = "Number of requests",
        help_heading = "Load Testing"
    )]
    pub max_num_completed_requests: u32,

    #[arg(
        long,
        default_value = "1",
        help = "Parallel requests",
        help_heading = "Load Testing"
    )]
    pub num_concurrent_requests: u32,

    #[arg(
        long,
        default_value = "600",
        help = "Request timeout",
        help_heading = "Load Testing"
    )]
    pub timeout: u64,

    // Tokenization
    #[arg(
        long,
        help = "Hugging Face tokenizer (defaults to model name)",
        help_heading = "Tokenization"
    )]
    pub tokenizer: Option<String>,

    #[arg(
        long,
        help = "Use server-reported token usage for metrics",
        help_heading = "Tokenization"
    )]
    pub use_server_token_count: bool,

    // Output
    #[arg(
        long,
        default_value = "result_outputs",
        help = "Output directory",
        help_heading = "Output"
    )]
    pub results_dir: String,

    #[arg(
        short = 'q',
        long,
        help = "Suppress stdout output",
        help_heading = "Output"
    )]
    pub quiet: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_default_api_type_is_chat() {
        let args = Args::try_parse_from([
            "llmnop",
            "--model",
            "test-model",
            "--url",
            "http://localhost:8000/v1",
            "--api-key",
            "test-key",
        ])
        .expect("parse args");

        assert!(matches!(args.api, ApiType::Chat));
    }

    #[test]
    fn test_parse_responses_api_type() {
        let args = Args::try_parse_from([
            "llmnop",
            "--api",
            "responses",
            "--model",
            "test-model",
            "--url",
            "http://localhost:8000/v1",
            "--api-key",
            "test-key",
        ])
        .expect("parse args");

        assert!(matches!(args.api, ApiType::Responses));
    }

    #[test]
    fn test_missing_url_is_error() {
        let result = Args::try_parse_from(["llmnop", "--model", "test-model", "--api-key", "key"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_api_key_is_allowed() {
        let args = Args::try_parse_from(["llmnop", "--model", "test-model", "--url", "http://x"])
            .expect("parse args");
        assert!(args.api_key.is_none());
    }

    #[test]
    fn test_default_quiet_is_false() {
        let args = Args::try_parse_from([
            "llmnop",
            "--model",
            "test-model",
            "--url",
            "http://localhost:8000/v1",
            "--api-key",
            "test-key",
        ])
        .expect("parse args");

        assert!(!args.quiet);
    }

    #[test]
    fn test_parse_quiet_flag() {
        let args = Args::try_parse_from([
            "llmnop",
            "--model",
            "test-model",
            "--url",
            "http://localhost:8000/v1",
            "--api-key",
            "test-key",
            "--quiet",
        ])
        .expect("parse args");

        assert!(args.quiet);
    }

    #[test]
    fn test_parse_quiet_short_flag() {
        let args = Args::try_parse_from([
            "llmnop",
            "--model",
            "test-model",
            "--url",
            "http://localhost:8000/v1",
            "--api-key",
            "test-key",
            "-q",
        ])
        .expect("parse args");

        assert!(args.quiet);
    }
}
