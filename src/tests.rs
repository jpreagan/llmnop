use super::*;
use crate::benchmark::Status;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

struct Reply {
    status: u16,
    chunks: Vec<(Duration, Vec<u8>)>,
}

impl Reply {
    fn sse(text: &str) -> Self {
        Self {
            status: 200,
            chunks: vec![(Duration::ZERO, text.as_bytes().to_vec())],
        }
    }
    fn delayed(first: &str, last: &str) -> Self {
        Self {
            status: 200,
            chunks: vec![
                (Duration::ZERO, first.as_bytes().to_vec()),
                (Duration::from_millis(150), last.as_bytes().to_vec()),
            ],
        }
    }
}

async fn server(
    replies: Vec<Reply>,
) -> (String, JoinHandle<Vec<(String, Value)>>, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let peak = Arc::new(AtomicUsize::new(0));
    let peak_clone = Arc::clone(&peak);
    let active = Arc::new(AtomicUsize::new(0));
    let task = tokio::spawn(async move {
        let mut jobs = JoinSet::new();
        for reply in replies {
            let (mut socket, _) = listener.accept().await.unwrap();
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak_clone);
            jobs.spawn(async move {
                let mut bytes = Vec::new();
                let mut buf = [0; 4096];
                let header_end = loop {
                    let n = socket.read(&mut buf).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buf[..n]);
                    if let Some(pos) = bytes.windows(4).position(|p| p == b"\r\n\r\n") { break pos + 4; }
                };
                let headers = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
                let content_length: usize = headers.lines().filter_map(|l| l.split_once(':')).find(|(k,_)| k.eq_ignore_ascii_case("content-length")).unwrap().1.trim().parse().unwrap();
                while bytes.len() < header_end + content_length {
                    let n = socket.read(&mut buf).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buf[..n]);
                }
                let body: Value = serde_json::from_slice(&bytes[header_end..header_end + content_length]).unwrap();
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(current, Ordering::SeqCst);
                let size: usize = reply.chunks.iter().map(|(_,b)| b.len()).sum();
                let head = format!("HTTP/1.1 {} Test\r\nContent-Type: text/event-stream\r\nContent-Length: {size}\r\nConnection: close\r\n\r\n", reply.status);
                if socket.write_all(head.as_bytes()).await.is_ok() {
                    for (delay, bytes) in reply.chunks {
                        if !delay.is_zero() { tokio::time::sleep(delay).await; }
                        if socket.write_all(&bytes).await.is_err() { break; }
                        tokio::task::yield_now().await;
                    }
                }
                active.fetch_sub(1, Ordering::SeqCst);
                (headers, body)
            });
        }
        let mut bodies = Vec::new();
        while let Some(job) = jobs.join_next().await {
            bodies.push(job.unwrap());
        }
        bodies
    });
    (url, task, peak)
}

fn prepared(client: &reqwest::Client, api: args::ApiType, url: &str, id: u32) -> PreparedRequest {
    let body = client::request_body(api, "test", "a b", Some(8), None, true);
    PreparedRequest {
        id,
        phase: Phase::Measurement,
        input_target: 2,
        input_tokens: 2,
        output_cap: Some(8),
        request: client::build_request(client, api, url, Some("test-secret"), &body).unwrap(),
    }
}

async fn finish_server(task: JoinHandle<Vec<(String, Value)>>) -> Vec<(String, Value)> {
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap()
}

const CONTENT: &str = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"a b\"}}]}\n\n";
const DONE: &str = "data: [DONE]\n\n";

#[tokio::test]
async fn all_three_wire_formats_preserve_local_counts_and_provider_usage() {
    for (api, payload) in [
        (
            args::ApiType::Chat,
            "data: {\"choices\":[{\"delta\":{\"content\":\"b c\",\"reasoning_content\":\"a\"},\"finish_reason\":\"length\"}]}\n\ndata: {\"choices\":[],\"usage\":{\"prompt_tokens\":99,\"completion_tokens\":88}}\n\ndata: [DONE]\n\n",
        ),
        (
            args::ApiType::Responses,
            "data: {\"type\":\"response.reasoning_summary_text.delta\",\"delta\":\"a\"}\n\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"b c\"}\n\ndata: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":99,\"output_tokens\":88}}}\n\n",
        ),
        (
            args::ApiType::Messages,
            "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":99,\"output_tokens\":0}}}\n\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"a\"}}\n\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"b c\"}}\n\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"max_tokens\"},\"usage\":{\"output_tokens\":88}}\n\ndata: {\"type\":\"message_stop\"}\n\n",
        ),
    ] {
        let (url, task, _) = server(vec![Reply::sse(payload)]).await;
        let client = client::http_client().unwrap();
        let mut extra = match api {
            args::ApiType::Chat => json!({"reasoning_effort": "high"}),
            args::ApiType::Responses => json!({"reasoning": {"effort": "high"}}),
            args::ApiType::Messages => {
                json!({"thinking": {"type": "enabled", "budget_tokens": 1024}})
            }
        };
        extra["temperature"] = json!(0.5);
        extra["vendor"] = json!({"enabled": true, "values": [1, "two", null], "model": "nested"});
        let config = Args::parse_from([
            "llmnop",
            "--url",
            &url,
            "--model",
            "test",
            "--api-key",
            "test-secret",
            "--api",
            match api {
                args::ApiType::Chat => "chat",
                args::ApiType::Responses => "responses",
                args::ApiType::Messages => "messages",
            },
            "--requests",
            "1",
            "--input-tokens",
            "2",
            "--output-cap",
            "8",
            "--request-usage",
            "--extra-inputs",
            &serde_json::to_string(&extra).unwrap(),
        ]);
        config.validate().unwrap();
        let tokenizer = tokens::test_tokenizer();
        let generator = PromptGenerator::new(&tokenizer).unwrap();
        let mut requests =
            prepare(&config, &client, &tokenizer, &generator, Phase::Measurement).unwrap();
        let (_tx, rx) = watch::channel(false);
        let record = benchmark::capture(
            client.clone(),
            api,
            requests.pop_front().unwrap(),
            Duration::from_secs(2),
            rx,
        )
        .await
        .finish(&tokens::test_tokenizer());
        assert_eq!(record.status, Status::Completed);
        assert_eq!(record.metrics.content_tokens, Some(2));
        assert_eq!(record.metrics.reasoning_tokens, Some(1));
        assert_eq!(record.metrics.generated_tokens, Some(3));
        assert!(record.metrics.ttft_ms.is_some());
        assert!(record.metrics.ttfo_ms.is_some());
        assert!(record.provider_usage.is_some());
        let data = serde_json::to_value(&record).unwrap();
        assert!(data.get("prompt").is_none());
        assert!(data.get("content").is_none());
        assert!(!data.to_string().contains("test-secret"));
        let wire = finish_server(task).await;
        let (headers, body) = &wire[0];
        assert_eq!(body["stream"], true);
        for (key, value) in extra.as_object().unwrap() {
            assert_eq!(&body[key], value);
        }
        let summary = BenchmarkSummary::new(&config, "test".into(), &[], false);
        let saved = serde_json::to_value(summary).unwrap();
        assert_eq!(saved["configuration"]["extra_inputs"], json!(extra));
        assert!(saved["configuration"].get("thinking_budget").is_none());
        assert!(headers.contains(if api == args::ApiType::Messages {
            "x-api-key: test-secret"
        } else {
            "authorization: Bearer test-secret"
        }));
        if api == args::ApiType::Messages {
            assert_eq!(record.provider_usage.as_ref().unwrap()["input_tokens"], 99);
        }
        if api == args::ApiType::Chat {
            assert_eq!(record.metrics.delivery_events, 1);
        }
    }
}

#[tokio::test]
async fn timeout_and_cancellation_preserve_partial_output() {
    for cancel in [false, true] {
        let (url, task, _) = server(vec![Reply::delayed(CONTENT, DONE)]).await;
        let client = client::http_client().unwrap();
        let (tx, rx) = watch::channel(false);
        let job = tokio::spawn(benchmark::capture(
            client.clone(),
            args::ApiType::Chat,
            prepared(&client, args::ApiType::Chat, &url, 0),
            Duration::from_millis(if cancel { 2000 } else { 70 }),
            rx,
        ));
        if cancel {
            tokio::time::sleep(Duration::from_millis(70)).await;
            tx.send(true).unwrap();
        }
        let record = job.await.unwrap().finish(&tokens::test_tokenizer());
        assert_eq!(
            record.status,
            if cancel {
                Status::Cancelled
            } else {
                Status::TimedOut
            }
        );
        assert_eq!(record.metrics.content_tokens, Some(2));
        assert!(record.metrics.ttft_ms.is_some());
        assert_eq!(record.metrics.request_latency_ms, None);
        assert!(record.elapsed_ms >= 50.0 && record.elapsed_ms < 1500.0);
        finish_server(task).await;
    }
}

#[tokio::test]
async fn timeout_before_output_has_missing_first_token_time() {
    let (url, task, _) = server(vec![Reply::delayed("", DONE)]).await;
    let client = client::http_client().unwrap();
    let (_tx, rx) = watch::channel(false);
    let r = benchmark::capture(
        client.clone(),
        args::ApiType::Chat,
        prepared(&client, args::ApiType::Chat, &url, 0),
        Duration::from_millis(30),
        rx,
    )
    .await
    .finish(&tokens::test_tokenizer());
    assert_eq!(r.status, Status::TimedOut);
    assert_eq!(r.metrics.ttft_ms, None);
    assert_eq!(r.metrics.generated_tokens, Some(0));
    finish_server(task).await;
}

#[tokio::test]
async fn truncated_and_malformed_streams_are_failures_even_after_text() {
    for ending in [
        "",
        "data: {not json}\n\n",
        "data: {\"error\":{\"message\":\"bad\"}}\n\n",
    ] {
        let (url, task, _) = server(vec![Reply::sse(&format!("{CONTENT}{ending}"))]).await;
        let client = client::http_client().unwrap();
        let (_tx, rx) = watch::channel(false);
        let r = benchmark::capture(
            client.clone(),
            args::ApiType::Chat,
            prepared(&client, args::ApiType::Chat, &url, 0),
            Duration::from_secs(2),
            rx,
        )
        .await
        .finish(&tokens::test_tokenizer());
        assert_eq!(r.status, Status::Failed);
        assert_eq!(r.metrics.content_tokens, Some(2));
        assert_eq!(r.metrics.request_latency_ms, None);
        finish_server(task).await;
    }
}

#[tokio::test]
async fn sse_handles_fragmented_utf8_crlf_comments_and_multiline_data() {
    let data = ": heartbeat\r\nevent: message\r\ndata: {\"choices\":\r\ndata: [{\"delta\":{\"content\":\"é\"}}]}\r\n\r\ndata: [DONE]\r\n\r\n";
    let chunks = data
        .as_bytes()
        .iter()
        .map(|b| (Duration::ZERO, vec![*b]))
        .collect();
    let (url, task, _) = server(vec![Reply {
        status: 200,
        chunks,
    }])
    .await;
    let client = client::http_client().unwrap();
    let (_tx, rx) = watch::channel(false);
    let r = benchmark::capture(
        client.clone(),
        args::ApiType::Chat,
        prepared(&client, args::ApiType::Chat, &url, 0),
        Duration::from_secs(2),
        rx,
    )
    .await
    .finish(&tokens::test_tokenizer());
    assert_eq!(r.status, Status::Completed);
    assert_eq!(r.metrics.delivery_events, 1);
    assert_eq!(r.metrics.content_tokens, Some(1));
    finish_server(task).await;
}

#[tokio::test]
async fn empty_and_tool_only_completions_have_no_text_timings() {
    for payload in [
        DONE.to_string(),
        format!(
            "data: {{\"choices\":[{{\"delta\":{{\"tool_calls\":[{{\"function\":{{\"arguments\":\"{{}}\"}}}}]}}}}]}}\n\n{DONE}"
        ),
    ] {
        let (url, task, _) = server(vec![Reply::sse(&payload)]).await;
        let client = client::http_client().unwrap();
        let (_tx, rx) = watch::channel(false);
        let r = benchmark::capture(
            client.clone(),
            args::ApiType::Chat,
            prepared(&client, args::ApiType::Chat, &url, 0),
            Duration::from_secs(2),
            rx,
        )
        .await
        .finish(&tokens::test_tokenizer());
        assert_eq!(r.status, Status::Completed);
        assert_eq!(r.metrics.generated_tokens, Some(0));
        assert_eq!(r.metrics.ttft_ms, None);
        assert!(r.metrics.request_latency_ms.is_some());
        finish_server(task).await;
    }
}

#[tokio::test]
async fn scheduler_bounds_concurrency_counts_failures_and_exports_recomputable_results() {
    let mut replies: Vec<_> = (0..5)
        .map(|_| Reply {
            status: 200,
            chunks: vec![(
                Duration::from_millis(40),
                format!("{CONTENT}{DONE}").into_bytes(),
            )],
        })
        .collect();
    replies[1].status = 503;
    let (url, task, peak) = server(replies).await;
    let args = Args::parse_from([
        "llmnop",
        "--url",
        &url,
        "--model",
        "test",
        "--requests",
        "5",
        "--concurrency",
        "2",
    ]);
    let client = client::http_client().unwrap();
    let tokenizer = Arc::new(tokens::test_tokenizer());
    let requests = (0..5)
        .map(|i| prepared(&client, args::ApiType::Chat, &url, i))
        .collect();
    let (_tx, rx) = watch::channel(false);
    let parent = std::env::temp_dir().join(format!("llmnop-test-{}", benchmark::unix_time_ns()));
    let mut writer = ResultsWriter::new(Some(&parent)).await.unwrap();
    let mut records = run_phase(&args, &client, &tokenizer, requests, &rx, &mut writer)
        .await
        .unwrap();
    assert_eq!(records.len(), 5);
    assert_eq!(peak.load(Ordering::SeqCst), 2);
    assert_eq!(
        records
            .iter()
            .filter(|r| r.status == Status::Completed)
            .count(),
        4
    );
    assert_eq!(
        records
            .iter()
            .filter(|r| r.http_status == Some(503))
            .count(),
        1
    );
    let mut ids: Vec<_> = records.iter().map(|r| r.request_id).collect();
    ids.sort_unstable();
    assert_eq!(ids, vec![0, 1, 2, 3, 4]);
    let summary = BenchmarkSummary::new(&args, writer.run_id.clone(), &records, false);
    writer.finish(&summary).await.unwrap();
    assert_eq!(summary.measurement.completed, 4);
    assert_eq!(summary.metrics["ttfo_ms"].count, 4);
    assert_eq!(summary.completed_token_totals.generated_tokens, 8);
    let exported = tokio::fs::read_to_string(writer.directory.join("requests.jsonl"))
        .await
        .unwrap();
    let rows: Vec<Value> = exported
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(rows.len(), 5);
    let total: u64 = rows
        .iter()
        .filter(|r| r["status"] == "completed")
        .map(|r| r["metrics"]["generated_tokens"].as_u64().unwrap())
        .sum();
    assert_eq!(total, summary.completed_token_totals.generated_tokens);
    let rate = total as f64 / (summary.measurement_duration_ms.unwrap() / 1000.0);
    assert_eq!(Some(rate), summary.completed_generated_tokens_per_second);
    let duration_ns = records.iter().map(|r| r.end_time_unix_ns).max().unwrap()
        - records.iter().map(|r| r.start_time_unix_ns).min().unwrap();
    assert!((summary.measurement_duration_ms.unwrap() - duration_ns as f64 / 1e6).abs() < 1e-9);
    records
        .iter_mut()
        .find(|r| r.status == Status::Completed)
        .unwrap()
        .phase = Phase::Warmup;
    let without_warmup = BenchmarkSummary::new(&args, writer.run_id.clone(), &records, false);
    assert_eq!(without_warmup.warmup.completed, 1);
    assert_eq!(without_warmup.measurement.completed, 3);
    assert_eq!(without_warmup.metrics["ttfo_ms"].count, 3);
    assert_eq!(without_warmup.completed_token_totals.generated_tokens, 6);
    assert_eq!(finish_server(task).await.len(), 5);
    tokio::fs::remove_dir_all(parent).await.unwrap();
}

#[test]
fn empty_summary_reports_missing_samples() {
    let args = Args::parse_from(["llmnop"]);
    let summary = BenchmarkSummary::new(&args, "test".into(), &[], true);
    let json = serde_json::to_value(summary).unwrap();
    assert_eq!(json["metrics"]["ttft_ms"]["count"], 0);
    assert_eq!(json["metrics"]["ttft_ms"]["mean"], Value::Null);
    assert_eq!(json["measurement_duration_ms"], Value::Null);
    assert_eq!(json["termination"], "interrupted");
    assert_eq!(json["configuration"]["api"], json!("chat"));
}

#[tokio::test]
async fn streamed_refusals_count_as_visible_content() {
    for (api, payload) in [
        (
            args::ApiType::Chat,
            "data: {\"choices\":[{\"delta\":{\"content\":null,\"refusal\":\"a b\"}}]}\n\ndata: [DONE]\n\n",
        ),
        (
            args::ApiType::Responses,
            "data: {\"type\":\"response.refusal.delta\",\"delta\":\"a b\"}\n\ndata: {\"type\":\"response.completed\"}\n\n",
        ),
    ] {
        let (url, task, _) = server(vec![Reply::sse(payload)]).await;
        let client = client::http_client().unwrap();
        let (_tx, rx) = watch::channel(false);
        let record = benchmark::capture(
            client.clone(),
            api,
            prepared(&client, api, &url, 0),
            Duration::from_secs(2),
            rx,
        )
        .await
        .finish(&tokens::test_tokenizer());
        assert_eq!(record.status, Status::Completed);
        assert_eq!(record.metrics.content_tokens, Some(2));
        assert!(record.metrics.ttfo_ms.is_some());
        finish_server(task).await;
    }
}

#[tokio::test]
async fn warmup_finishes_before_measurement_and_deadlines_free_slots() {
    let replies = vec![
        Reply::sse(&format!("{CONTENT}{DONE}")),
        Reply::sse(&format!("{CONTENT}{DONE}")),
        Reply::delayed(CONTENT, DONE),
        Reply::sse(&format!("{CONTENT}{DONE}")),
    ];
    let (url, task, _) = server(replies).await;
    let args = Args::parse_from([
        "llmnop",
        "--url",
        &url,
        "--model",
        "test",
        "--requests",
        "2",
        "--concurrency",
        "1",
        "--warmup",
        "2",
        "--request-timeout",
        "0.1",
    ]);
    let client = client::http_client().unwrap();
    let tokenizer = Arc::new(tokens::test_tokenizer());
    let (_tx, rx) = watch::channel(false);
    let parent = std::env::temp_dir().join(format!("llmnop-phases-{}", benchmark::unix_time_ns()));
    let mut writer = ResultsWriter::new(Some(&parent)).await.unwrap();
    let warmup = (0..2)
        .map(|i| {
            let mut r = prepared(&client, args::ApiType::Chat, &url, i);
            r.phase = Phase::Warmup;
            r
        })
        .collect();
    let warmup = run_phase(&args, &client, &tokenizer, warmup, &rx, &mut writer)
        .await
        .unwrap();
    assert!(
        warmup
            .iter()
            .all(|r| r.phase == Phase::Warmup && r.status == Status::Completed)
    );
    let measured = (2..4)
        .map(|i| prepared(&client, args::ApiType::Chat, &url, i))
        .collect();
    let measured = run_phase(&args, &client, &tokenizer, measured, &rx, &mut writer)
        .await
        .unwrap();
    assert_eq!(measured.len(), 2);
    assert!(
        warmup.iter().map(|r| r.end).max().unwrap()
            <= measured.iter().map(|r| r.start).min().unwrap()
    );
    let timed_out = measured
        .iter()
        .find(|r| r.status == Status::TimedOut)
        .unwrap();
    assert_eq!(timed_out.request_id, 2);
    assert_eq!(timed_out.metrics.content_tokens, Some(2));
    assert!(
        measured
            .iter()
            .any(|r| r.request_id == 3 && r.status == Status::Completed)
    );
    let exported = tokio::fs::read_to_string(writer.directory.join("requests.jsonl"))
        .await
        .unwrap();
    assert_eq!(exported.lines().count(), 4);
    assert_eq!(finish_server(task).await.len(), 4);
    tokio::fs::remove_dir_all(parent).await.unwrap();
}
