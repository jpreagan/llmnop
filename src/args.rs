#[cfg(feature = "self-update")]
use clap::Subcommand;
use clap::builder::styling::{AnsiColor, Effects};
use clap::builder::Styles;
use clap::error::ErrorKind;
use clap::{CommandFactory, Parser, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ApiType {
    Chat,
    Responses,
}

#[cfg(feature = "self-update")]
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Update llmnop (standalone installs only)
    Update,
}

const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default());

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, styles = STYLES)]
#[cfg_attr(feature = "self-update", command(subcommand_negates_reqs = true))]
pub struct Args {
    #[cfg(feature = "self-update")]
    #[command(subcommand)]
    pub command: Option<Command>,

    // Endpoint
    #[arg(
        long,
        help = "Base URL (e.g., http://localhost:8000/v1)",
        help_heading = "Endpoint"
    )]
    pub url: Option<String>,

    #[arg(long, help = "API key", help_heading = "Endpoint")]
    pub api_key: Option<String>,

    #[arg(short, long, help = "Model name", help_heading = "Endpoint")]
    pub model: Option<String>,

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

impl Args {
    pub fn require_benchmark_args(&self) -> Result<(&str, &str), clap::Error> {
        let url = self
            .url
            .as_deref()
            .ok_or_else(|| Self::missing_required_arg("--url"))?;
        let model = self
            .model
            .as_deref()
            .ok_or_else(|| Self::missing_required_arg("--model"))?;
        Ok((url, model))
    }

    fn missing_required_arg(arg: &str) -> clap::Error {
        Self::command().error(
            ErrorKind::MissingRequiredArgument,
            format!("the following required argument was not provided: {arg}"),
        )
    }
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
        let args = Args::try_parse_from(["llmnop", "--model", "test-model", "--api-key", "key"])
            .expect("parse args");
        assert!(args.require_benchmark_args().is_err());
    }

    #[test]
    fn test_missing_model_is_error() {
        let args = Args::try_parse_from(["llmnop", "--url", "http://localhost:8000/v1"])
            .expect("parse args");
        assert!(args.require_benchmark_args().is_err());
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

    #[cfg(feature = "self-update")]
    #[test]
    fn test_parse_update_command() {
        let args = Args::try_parse_from(["llmnop", "update"]).expect("parse args");
        assert!(matches!(args.command, Some(Command::Update)));
    }
}
