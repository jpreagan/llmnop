#[cfg(feature = "self-update")]
use clap::Subcommand;
use clap::builder::Styles;
use clap::builder::styling::{AnsiColor, Effects};
use clap::error::ErrorKind;
use clap::{CommandFactory, Parser, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum ApiType {
    Chat,
    Responses,
    Messages,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum OutputFormat {
    Table,
    Json,
    None,
}

#[cfg(feature = "self-update")]
#[derive(Debug, Subcommand)]
pub enum Command {
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

    #[arg(
        long,
        default_value = "550",
        help = "Target input length",
        help_heading = "Request Shaping"
    )]
    pub input_tokens: u32,

    #[arg(
        long,
        default_value = "0",
        help = "Input length variance",
        help_heading = "Request Shaping"
    )]
    pub input_tokens_stddev: u32,

    #[arg(
        long,
        help = "Mean output token cap to request [default: none]",
        help_heading = "Request Shaping"
    )]
    pub output_cap: Option<u32>,

    #[arg(
        long,
        default_value = "0",
        help = "Output length variance",
        help_heading = "Request Shaping"
    )]
    pub output_cap_stddev: u32,

    #[arg(
        long,
        help = "Enable Anthropic Messages thinking with this token budget",
        help_heading = "Request Shaping"
    )]
    pub thinking_budget: Option<u32>,

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

    #[arg(
        long,
        help = "Hugging Face tokenizer (defaults to model name)",
        help_heading = "Tokenization"
    )]
    pub tokenizer: Option<String>,

    #[arg(
        long,
        help = "Request provider-reported token usage and record it separately",
        help_heading = "Tokenization"
    )]
    pub use_server_token_count: bool,

    #[arg(
        long,
        value_enum,
        default_value = "table",
        help = "Stdout output format",
        help_heading = "Output"
    )]
    pub output_format: OutputFormat,

    #[arg(
        long,
        help = "Emit benchmark summary JSON to stdout (alias for --output-format json)",
        help_heading = "Output"
    )]
    pub json: bool,

    #[arg(
        short = 'q',
        long,
        help = "Suppress stdout output (alias for --output-format none)",
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

    pub fn effective_output_format(&self) -> OutputFormat {
        if self.quiet {
            OutputFormat::None
        } else if self.json {
            OutputFormat::Json
        } else {
            self.output_format
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if matches!(self.api, ApiType::Messages) && self.output_cap.is_none() {
            return Err("--output-cap is required for --api messages".to_string());
        }
        if let Some(thinking_budget) = self.thinking_budget {
            if !matches!(self.api, ApiType::Messages) {
                return Err("--thinking-budget requires --api messages".to_string());
            }
            if thinking_budget < 1024 {
                return Err("--thinking-budget must be at least 1024".to_string());
            }
            let output_cap = self
                .output_cap
                .expect("--output-cap is required for --api messages");
            if output_cap <= thinking_budget {
                return Err("--output-cap must be greater than --thinking-budget".to_string());
            }
        }
        Ok(())
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
    fn test_parse_messages_api_type() {
        let args = Args::try_parse_from([
            "llmnop",
            "--api",
            "messages",
            "--model",
            "test-model",
            "--url",
            "https://api.anthropic.com/v1",
            "--api-key",
            "test-key",
        ])
        .expect("parse args");

        assert!(matches!(args.api, ApiType::Messages));
    }

    #[test]
    fn test_messages_api_requires_output_cap() {
        let args = Args::try_parse_from([
            "llmnop",
            "--api",
            "messages",
            "--model",
            "test-model",
            "--url",
            "https://api.anthropic.com/v1",
            "--api-key",
            "test-key",
        ])
        .expect("parse args");

        assert!(args.validate().is_err());
    }

    #[test]
    fn test_messages_api_allows_output_cap() {
        let args = Args::try_parse_from([
            "llmnop",
            "--api",
            "messages",
            "--model",
            "test-model",
            "--url",
            "https://api.anthropic.com/v1",
            "--api-key",
            "test-key",
            "--output-cap",
            "150",
        ])
        .expect("parse args");

        assert!(args.validate().is_ok());
    }

    #[test]
    fn test_thinking_budget_requires_messages_api() {
        let args = Args::try_parse_from([
            "llmnop",
            "--api",
            "chat",
            "--model",
            "test-model",
            "--url",
            "http://localhost:8000/v1",
            "--api-key",
            "test-key",
            "--output-cap",
            "1500",
            "--thinking-budget",
            "1024",
        ])
        .expect("parse args");

        assert!(args.validate().is_err());
    }

    #[test]
    fn test_thinking_budget_requires_at_least_1024_tokens() {
        let args = Args::try_parse_from([
            "llmnop",
            "--api",
            "messages",
            "--model",
            "test-model",
            "--url",
            "https://api.anthropic.com/v1",
            "--api-key",
            "test-key",
            "--output-cap",
            "1500",
            "--thinking-budget",
            "1023",
        ])
        .expect("parse args");

        assert!(args.validate().is_err());
    }

    #[test]
    fn test_thinking_budget_requires_larger_output_cap() {
        let args = Args::try_parse_from([
            "llmnop",
            "--api",
            "messages",
            "--model",
            "test-model",
            "--url",
            "https://api.anthropic.com/v1",
            "--api-key",
            "test-key",
            "--output-cap",
            "1024",
            "--thinking-budget",
            "1024",
        ])
        .expect("parse args");

        assert!(args.validate().is_err());
    }

    #[test]
    fn test_thinking_budget_allows_messages_api() {
        let args = Args::try_parse_from([
            "llmnop",
            "--api",
            "messages",
            "--model",
            "test-model",
            "--url",
            "https://api.anthropic.com/v1",
            "--api-key",
            "test-key",
            "--output-cap",
            "1500",
            "--thinking-budget",
            "1024",
        ])
        .expect("parse args");

        assert!(args.validate().is_ok());
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
        assert!(!args.json);
        assert!(matches!(args.output_format, OutputFormat::Table));
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
        assert!(matches!(args.effective_output_format(), OutputFormat::None));
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
        assert!(matches!(args.effective_output_format(), OutputFormat::None));
    }

    #[test]
    fn test_parse_output_format_json() {
        let args = Args::try_parse_from([
            "llmnop",
            "--model",
            "test-model",
            "--url",
            "http://localhost:8000/v1",
            "--api-key",
            "test-key",
            "--output-format",
            "json",
        ])
        .expect("parse args");

        assert!(matches!(args.output_format, OutputFormat::Json));
        assert!(matches!(args.effective_output_format(), OutputFormat::Json));
    }

    #[test]
    fn test_parse_json_flag() {
        let args = Args::try_parse_from([
            "llmnop",
            "--model",
            "test-model",
            "--url",
            "http://localhost:8000/v1",
            "--api-key",
            "test-key",
            "--json",
        ])
        .expect("parse args");

        assert!(args.json);
        assert!(matches!(args.effective_output_format(), OutputFormat::Json));
    }

    #[test]
    fn test_quiet_overrides_output_format() {
        let args = Args::try_parse_from([
            "llmnop",
            "--model",
            "test-model",
            "--url",
            "http://localhost:8000/v1",
            "--api-key",
            "test-key",
            "--output-format",
            "json",
            "--quiet",
        ])
        .expect("parse args");

        assert!(matches!(args.output_format, OutputFormat::Json));
        assert!(matches!(args.effective_output_format(), OutputFormat::None));
    }

    #[test]
    fn test_quiet_overrides_json_flag() {
        let args = Args::try_parse_from([
            "llmnop",
            "--model",
            "test-model",
            "--url",
            "http://localhost:8000/v1",
            "--api-key",
            "test-key",
            "--json",
            "--quiet",
        ])
        .expect("parse args");

        assert!(args.json);
        assert!(args.quiet);
        assert!(matches!(args.effective_output_format(), OutputFormat::None));
    }

    #[cfg(feature = "self-update")]
    #[test]
    fn test_parse_update_command() {
        let args = Args::try_parse_from(["llmnop", "update"]).expect("parse args");
        assert!(matches!(args.command, Some(Command::Update)));
    }
}
