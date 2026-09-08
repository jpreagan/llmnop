use super::*;
use crate::benchmark::Status;
use crate::benchmark::{Phase, PreparedRequest};
use serde_json::Value;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::task::JoinSet;

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
