use clap::builder::Styles;
use clap::builder::styling::{AnsiColor, Effects};
use clap::error::ErrorKind;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
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

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Update llmnop (standalone installs only)")]
    Update,
}

const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default());

#[derive(Parser, Debug, Serialize)]
#[command(version, about, long_about = None, styles = STYLES, subcommand_negates_reqs = true)]
pub struct Args {
    #[command(subcommand)]
    #[serde(skip)]
    pub command: Option<Command>,
    #[arg(
        long,
        value_name = "URL",
        help = "API base URL, including its version prefix",
        help_heading = "Endpoint"
    )]
    pub url: Option<String>,
    #[arg(
        short,
        long,
        help = "Model identifier sent to the endpoint",
        help_heading = "Endpoint"
    )]
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
        value_name = "KEY",
        help = "Authentication credential",
        help_heading = "Endpoint"
    )]
    #[serde(skip)]
    pub api_key: Option<String>,
    #[arg(
        long,
        help = "Tokenizer for prompt sizing and local counts [default: model identifier]",
        help_heading = "Endpoint"
    )]
    pub tokenizer: Option<String>,
    #[arg(long, value_name = "N", default_value_t = 550, value_parser = clap::value_parser!(u32).range(1..), help = "Mean prompt-text token target", help_heading = "Workload")]
    pub input_tokens: u32,
    #[arg(
        long,
        value_name = "N",
        default_value_t = 0,
        help = "Prompt-target standard deviation",
        help_heading = "Workload"
    )]
    pub input_tokens_stddev: u32,
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u32).range(1..), help = "Mean requested generation-token cap [default: omitted; required for messages]", help_heading = "Workload")]
    pub output_cap: Option<u32>,
    #[arg(
        long,
        value_name = "N",
        default_value_t = 0,
        help = "Generation-cap standard deviation",
        help_heading = "Workload"
    )]
    pub output_cap_stddev: u32,
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u32).range(1024..), help = "Messages API thinking-token budget", help_heading = "Workload")]
    pub thinking_budget: Option<u32>,
    #[arg(short = 'n', long, value_name = "N", default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..), help = "Measured request attempts", help_heading = "Run")]
    pub requests: u32,
    #[arg(short = 'c', long, value_name = "N", default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..), help = "Maximum simultaneous requests", help_heading = "Run")]
    pub concurrency: u32,
    #[arg(
        long,
        value_name = "N",
        default_value_t = 0,
        help = "Total warmup attempts",
        help_heading = "Run"
    )]
    pub warmup: u32,
    #[arg(
        long,
        value_name = "SECONDS",
        default_value_t = 600.0,
        help = "Deadline for each complete request",
        help_heading = "Run"
    )]
    pub request_timeout: f64,
    #[arg(long, value_enum, default_value = "table", help_heading = "Output")]
    #[serde(skip)]
    pub output_format: OutputFormat,
    #[arg(long, help = "Emit summary JSON", help_heading = "Output")]
    #[serde(skip)]
    pub json: bool,
    #[arg(short = 'q', long, help = "Suppress stdout", help_heading = "Output")]
    #[serde(skip)]
    pub quiet: bool,
    #[arg(
        long,
        help = "Request provider-reported usage",
        help_heading = "Output"
    )]
    pub use_server_token_count: bool,
}

impl Args {
    pub fn effective_output_format(&self) -> OutputFormat {
        if self.quiet {
            OutputFormat::None
        } else if self.json {
            OutputFormat::Json
        } else {
            self.output_format
        }
    }

    pub fn validate(&self) -> Result<(), clap::Error> {
        let invalid = |message: &str| Self::command().error(ErrorKind::ValueValidation, message);
        for (name, value) in [("--url", &self.url), ("--model", &self.model)] {
            if value.as_ref().is_none_or(|s| s.trim().is_empty()) {
                return Err(Self::command().error(
                    ErrorKind::MissingRequiredArgument,
                    format!("{name} is required for benchmarks"),
                ));
            }
        }
        let url = reqwest::Url::parse(self.url.as_deref().unwrap())
            .map_err(|_| invalid("--url must be an HTTP(S) base URL"))?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err(invalid("--url must be an HTTP(S) base URL"));
        }
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(invalid(
                "--url must not contain credentials, a query, or a fragment",
            ));
        }
        if self.api == ApiType::Messages && self.output_cap.is_none() {
            return Err(invalid("--output-cap is required for --api messages"));
        }
        if self.output_cap_stddev > 0 && self.output_cap.is_none() {
            return Err(invalid("--output-cap-stddev requires --output-cap"));
        }
        if let Some(budget) = self.thinking_budget {
            if self.api != ApiType::Messages {
                return Err(invalid("--thinking-budget requires --api messages"));
            }
            if self.output_cap.is_none_or(|cap| cap <= budget) {
                return Err(invalid("--output-cap must exceed --thinking-budget"));
            }
        }
        if Duration::try_from_secs_f64(self.request_timeout).is_err() || self.request_timeout <= 0.0
        {
            return Err(invalid(
                "--request-timeout must be a finite positive duration",
            ));
        }
        if self.requests.checked_add(self.warmup).is_none() {
            return Err(invalid(
                "--requests plus --warmup exceeds the supported request count",
            ));
        }
        if std::time::Instant::now()
            .checked_add(Duration::from_secs_f64(self.request_timeout))
            .is_none()
        {
            return Err(invalid("--request-timeout exceeds the supported duration"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_workloads_before_connecting() {
        for flags in [
            vec!["--requests", "0"],
            vec!["--concurrency", "0"],
            vec!["--input-tokens", "0"],
            vec!["--output-cap", "0"],
            vec!["--api", "messages"],
            vec!["--thinking-budget", "1024"],
            vec!["--output-cap-stddev", "1"],
            vec!["--request-timeout", "NaN"],
            vec!["--request-timeout", "0"],
            vec![
                "--api",
                "messages",
                "--output-cap",
                "1024",
                "--thinking-budget",
                "1024",
            ],
        ] {
            let mut argv = vec!["llmnop", "--url", "http://localhost/v1", "--model", "test"];
            argv.extend(flags);
            assert!(Args::try_parse_from(argv).map_or(true, |args| args.validate().is_err()));
        }
    }

    #[test]
    fn help_and_update_do_not_require_an_endpoint() {
        let help = Args::try_parse_from(["llmnop", "--help"]).unwrap_err();
        assert_eq!(help.kind(), ErrorKind::DisplayHelp);
        assert!(help.to_string().contains("standalone installs only"));
        assert!(!help.to_string().contains("env:"));
        assert!(Args::try_parse_from(["llmnop", "update"]).is_ok());
    }

    #[test]
    fn config_never_serializes_credentials() {
        let args = Args::parse_from(["llmnop", "--api-key", "secret"]);
        assert!(!serde_json::to_string(&args).unwrap().contains("secret"));
    }
}
