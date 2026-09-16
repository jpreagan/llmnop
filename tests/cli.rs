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
use support::process::Terminal;
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

    fn command(&self, server: &Server, api: &str, format: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_llmnop"));
        command
            .args([
                "--url",
                server.url(),
                "--api",
                api,
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
}

fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn terminal_progress_preserves_requests_and_machine_readable_results() {
    for (api, path, cap) in [
        ("chat", "/v1/chat/completions", "max_completion_tokens"),
        ("responses", "/v1/responses", "max_output_tokens"),
        ("messages", "/v1/messages", "max_tokens"),
    ] {
        for format in ["json", "none"] {
            let server = Server::new(
                api,
                (0..3).map(|_| Reply::Text("hello world".into())).collect(),
            );
            let fixture = Fixture::new();
            let mut command = fixture.command(&server, api, format);
            command.args(["--requests", "2", "--warmup", "1", "--concurrency", "2"]);
            let output = Terminal::spawn(&mut command, 80, 24, false).finish();
            assert!(output.status.success(), "{}", output.visible);
            let (_, summary, records) = fixture.results();
            if format == "json" {
                assert_eq!(
                    serde_json::from_slice::<Value>(&output.stdout).unwrap(),
                    summary
                );
            } else {
                assert!(output.stdout.is_empty());
            }
            let requests = server.requests();
            assert_eq!(requests.len(), 3);
            for request in requests {
                assert_eq!(request.path, path);
                assert_eq!(request.body["model"], "integration-test-model");
                assert_eq!(request.body["stream"], true);
                assert_eq!(request.body[cap], 16);
                let prompt = if api == "responses" {
                    &request.body["input"]
                } else {
                    &request.body["messages"][0]["content"]
                };
                assert_eq!(prompt.as_str().unwrap().split_whitespace().count(), 2);
            }
            assert_eq!(summary["warmup"]["completed"], 1);
            assert_eq!(summary["measurement"]["completed"], 2);
            assert_eq!(summary["completed_token_totals"]["generated_tokens"], 4);
            assert_eq!(records.len(), 3);
            assert_eq!(records.iter().filter(|r| r["phase"] == "warmup").count(), 1);
            let mut ids: Vec<_> = records
                .iter()
                .map(|r| r["request_id"].as_u64().unwrap())
                .collect();
            ids.sort_unstable();
            assert_eq!(ids, [0, 1, 2]);
            for record in records {
                assert_eq!(record["status"], "completed");
                assert_eq!(record["metrics"]["input_tokens"], 2);
                assert_eq!(record["metrics"]["generated_tokens"], 2);
            }
        }
    }
}

#[test]
fn failed_requests_keep_complete_diagnostics_and_results() {
    let diagnostic = format!(
        "Invalid configuration: {} Remove unsupported parameter {}.",
        "測定 requires a supported parameter value. ".repeat(40),
        "setting_".repeat(16),
    );
    for (columns, rows) in [(80, 24), (32, 6)] {
        let server = Server::new(
            "responses",
            vec![
                Reply::Failure(diagnostic.clone()),
                Reply::Text("hello world".into()),
            ],
        );
        let fixture = Fixture::new();
        let mut command = fixture.command(&server, "responses", "json");
        command.args(["--requests", "2"]);
        let output = Terminal::spawn(&mut command, columns, rows, false).finish();
        assert_eq!(output.status.code(), Some(1));
        assert!(
            compact(&output.visible).contains(&compact(&diagnostic)),
            "{}",
            output.visible
        );
        let (_, summary, records) = fixture.results();
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap(),
            summary
        );
        assert_eq!(server.requests().len(), 2);
        assert_eq!(summary["measurement"]["failed"], 1);
        assert_eq!(summary["measurement"]["completed"], 1);
        assert_eq!(records.len(), 2);
        let failed = records.iter().find(|r| r["status"] == "failed").unwrap();
        assert_eq!(failed["http_status"], 400);
        assert!(
            failed["error"]["message"]
                .as_str()
                .unwrap()
                .contains(&diagnostic)
        );
    }
}

#[test]
fn unsupported_terminals_preserve_results_without_control_sequences() {
    for term in [Some("dumb"), None, Some("")] {
        let server = Server::new("chat", vec![Reply::Text("hello world".into())]);
        let fixture = Fixture::new();
        let mut command = fixture.command(&server, "chat", "json");
        command.args(["--requests", "1"]);
        match term {
            Some(term) => {
                command.env("TERM", term);
            }
            None => {
                command.env_remove("TERM");
            }
        }
        let output = Terminal::spawn(&mut command, 80, 24, false).finish();
        assert!(output.status.success(), "{}", output.visible);
        assert!(!output.stderr.contains(&0x1b));
        let (_, summary, records) = fixture.results();
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap(),
            summary
        );
        assert_eq!(summary["measurement"]["completed"], 1);
        assert_eq!(records.len(), 1);
        assert_eq!(server.requests().len(), 1);
    }
}

#[test]
fn interrupt_saves_started_requests_and_stops_admitting_work() {
    let server = Server::new("chat", vec![Reply::Hold]);
    let fixture = Fixture::new();
    let mut command = fixture.command(&server, "chat", "json");
    command.args(["--requests", "4", "--concurrency", "1"]);
    let mut terminal = Terminal::spawn(&mut command, 80, 24, false);
    assert!(server.wait_for_requests(1, Duration::from_secs(20)));
    terminal.interrupt();
    let output = terminal.finish();
    assert_eq!(output.status.code(), Some(130), "{}", output.visible);
    let (_, summary, records) = fixture.results();
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        summary
    );
    assert_eq!(summary["termination"], "interrupted");
    assert_eq!(summary["measurement"]["cancelled"], 1);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["status"], "cancelled");
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn human_report_keeps_the_saved_results_discoverable() {
    let server = Server::new("responses", vec![Reply::Text("hello world".into())]);
    let fixture = Fixture::new();
    let mut command = fixture.command(&server, "responses", "table");
    command.args(["--requests", "1"]);
    let output = Terminal::spawn(&mut command, 60, 6, true).finish();
    assert!(output.status.success(), "{}", output.visible);
    let (directory, summary, _) = fixture.results();
    assert_eq!(summary["measurement"]["completed"], 1);
    assert!(compact(&output.visible).contains(&compact(&directory.display().to_string())));
}
