#![cfg(unix)]

mod support {
    pub mod process;
    pub mod server;
}

use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use support::process::{Output, Terminal};
use support::server::{Reply, Server};
use tempfile::TempDir;

struct Fixture {
    directory: TempDir,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let tokenizer = json!({
            "version": "1.0", "truncation": null, "padding": null, "added_tokens": [],
            "normalizer": null, "pre_tokenizer": {"type": "WhitespaceSplit"},
            "post_processor": null, "decoder": null,
            "model": {
                "type": "WordLevel", "vocab": {"[UNK]": 0, "hello": 1, "world": 2},
                "unk_token": "[UNK]"
            }
        });
        fs::write(
            directory.path().join("tokenizer.json"),
            tokenizer.to_string(),
        )
        .unwrap();
        Self { directory }
    }

    fn command(&self, server: &Server, format: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_llmnop"));
        command
            .args([
                "--url",
                server.url(),
                "--model",
                "integration-test-model",
                "--input-tokens",
                "2",
                "--output-cap",
                "16",
                "--format",
                format,
            ])
            .arg("--tokenizer")
            .arg(self.directory.path().join("tokenizer.json"))
            .arg("--results-dir")
            .arg(self.directory.path().join("results"))
            .env("TERM", "xterm-256color")
            .env_remove("NO_COLOR")
            .env("NO_PROXY", "127.0.0.1")
            .env("no_proxy", "127.0.0.1")
            .env("RAYON_NUM_THREADS", "1")
            .env("TOKIO_WORKER_THREADS", "2");
        command
    }

    fn results(&self) -> (PathBuf, Value, Vec<Value>) {
        let runs: Vec<_> = fs::read_dir(self.directory.path().join("results"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(runs.len(), 1);
        let directory = &runs[0];
        let summary =
            serde_json::from_slice(&fs::read(directory.join("summary.json")).unwrap()).unwrap();
        let records = fs::read_to_string(directory.join("requests.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        (directory.clone(), summary, records)
    }

    fn summary(&self, output: &Output) -> (Value, Vec<Value>) {
        let (_, summary, records) = self.results();
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap(),
            summary
        );
        (summary, records)
    }
}

fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn report_is_visible_after_the_live_view_closes() {
    let server = Server::new((0..3).map(|_| Reply::Text("hello world".into())).collect());
    let fixture = Fixture::new();
    let mut command = fixture.command(&server, "table");
    command.args(["--requests", "2", "--warmup", "1", "--concurrency", "2"]);
    let output = Terminal::spawn(&mut command, 80, 24, true).finish();
    assert!(output.status.success(), "{}", output.visible);
    let (directory, summary, _) = fixture.results();
    assert_eq!(summary["warmup"]["completed"], 1);
    assert_eq!(summary["measurement"]["completed"], 2);
    assert!(compact(&output.visible).contains(&compact(&directory.display().to_string())));
}

#[test]
fn failed_requests_are_listed_in_full_after_the_live_view_closes() {
    let diagnostic = format!(
        "Invalid configuration: {} Remove unsupported parameter {}.",
        "測定 requires a supported parameter value. ".repeat(40),
        "setting_".repeat(16),
    );
    let server = Server::new(vec![
        Reply::Failure(diagnostic.clone()),
        Reply::Text("hello world".into()),
    ]);
    let fixture = Fixture::new();
    let mut command = fixture.command(&server, "json");
    command.args(["--requests", "2"]);
    let output = Terminal::spawn(&mut command, 80, 24, false).finish();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        compact(&output.visible).contains(&compact(&diagnostic)),
        "{}",
        output.visible
    );
    let (summary, _) = fixture.summary(&output);
    assert_eq!(server.received(), 2);
    assert_eq!(summary["measurement"]["failed"], 1);
    assert_eq!(summary["measurement"]["completed"], 1);
}

#[test]
fn ctrl_c_cancels_the_run_and_saves_results() {
    let server = Server::new(vec![Reply::Hold]);
    let fixture = Fixture::new();
    let mut command = fixture.command(&server, "json");
    command.args(["--requests", "4", "--concurrency", "1"]);
    let mut terminal = Terminal::spawn(&mut command, 80, 24, false);
    assert!(server.wait_for_requests(1, Duration::from_secs(20)));
    terminal.interrupt();
    let output = terminal.finish();
    assert_eq!(output.status.code(), Some(130), "{}", output.visible);
    let (summary, records) = fixture.summary(&output);
    assert_eq!(summary["termination"], "interrupted");
    assert_eq!(summary["measurement"]["cancelled"], 1);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["status"], "cancelled");
    assert_eq!(server.received(), 1);
}

#[test]
fn basic_terminals_get_plain_text_progress() {
    for term in [Some("dumb"), None, Some("")] {
        let server = Server::new(vec![Reply::Text("hello world".into())]);
        let fixture = Fixture::new();
        let mut command = fixture.command(&server, "json");
        command.args(["--requests", "1"]);
        match term {
            Some(term) => command.env("TERM", term),
            None => command.env_remove("TERM"),
        };
        let output = Terminal::spawn(&mut command, 80, 24, false).finish();
        assert!(output.status.success(), "{}", output.visible);
        assert!(!output.stderr.contains(&0x1b), "{term:?}");
        let (summary, _) = fixture.summary(&output);
        assert_eq!(summary["measurement"]["completed"], 1);
    }
}
