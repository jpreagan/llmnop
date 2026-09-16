use serde_json::{Value, json};
use std::io;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::JoinSet;

pub enum Reply {
    Text(String),
    Failure(String),
    Hold,
}

#[derive(Clone, Debug)]
pub struct Request {
    pub path: String,
    pub body: Value,
}

type Requests = Arc<(Mutex<Vec<Request>>, Condvar)>;

pub struct Server {
    url: String,
    requests: Requests,
    shutdown: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    pub fn new(api: &str, replies: Vec<Reply>) -> Self {
        assert!(matches!(api, "chat" | "responses" | "messages"));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/v1", listener.local_addr().unwrap());
        let requests = Arc::new((Mutex::new(Vec::new()), Condvar::new()));
        let received = Arc::clone(&requests);
        let api = api.to_owned();
        let (shutdown, mut stopped) = oneshot::channel();
        let thread = thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let listener = TcpListener::from_std(listener).unwrap();
                let mut replies = replies.into_iter();
                let mut connections = JoinSet::new();
                loop {
                    tokio::select! {
                        _ = &mut stopped => break,
                        accepted = listener.accept() => {
                            let (stream, _) = accepted.unwrap();
                            let reply = replies.next();
                            let api = api.clone();
                            let requests = Arc::clone(&received);
                            connections.spawn(async move {
                                // A cancelled benchmark may close the connection during a write.
                                let _ = respond(stream, &api, reply, requests).await;
                            });
                        }
                        _ = connections.join_next(), if !connections.is_empty() => {}
                    }
                }
                connections.shutdown().await;
            });
        });
        Self {
            url,
            requests,
            shutdown: Some(shutdown),
            thread: Some(thread),
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn requests(&self) -> Vec<Request> {
        self.requests.0.lock().unwrap().clone()
    }

    pub fn wait_for_requests(&self, count: usize, timeout: Duration) -> bool {
        let (requests, changed) = &*self.requests;
        let (requests, _) = changed
            .wait_timeout_while(requests.lock().unwrap(), timeout, |r| r.len() < count)
            .unwrap();
        requests.len() >= count
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.shutdown.take().unwrap().send(());
        let _ = self.thread.take().unwrap().join();
    }
}

async fn read_request(stream: TcpStream) -> io::Result<(TcpStream, Request)> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    let path = line
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_owned();
    let mut length = None;
    loop {
        line.clear();
        if reader.read_line(&mut line).await? == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        if line == "\r\n" {
            break;
        }
        if let Some((_, value)) = line
            .split_once(':')
            .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        {
            length = Some(value.trim().parse::<usize>().map_err(io::Error::other)?);
        }
    }
    let mut body = vec![0; length.ok_or_else(|| io::Error::other("missing content length"))?];
    reader.read_exact(&mut body).await?;
    let body = serde_json::from_slice(&body).map_err(io::Error::other)?;
    Ok((reader.into_inner(), Request { path, body }))
}

async fn respond(
    stream: TcpStream,
    api: &str,
    reply: Option<Reply>,
    requests: Requests,
) -> io::Result<()> {
    let (mut stream, request) =
        tokio::time::timeout(Duration::from_secs(5), read_request(stream)).await??;
    requests.0.lock().unwrap().push(request);
    requests.1.notify_all();

    let (status, content_type, body, hold) = match reply {
        Some(Reply::Failure(message)) => ("400 Bad Request", "text/plain", message, false),
        None => (
            "500 Internal Server Error",
            "text/plain",
            "unexpected request".to_owned(),
            false,
        ),
        Some(reply) => {
            let (text, hold) = match reply {
                Reply::Text(text) => (text, false),
                Reply::Hold => ("hello".to_owned(), true),
                Reply::Failure(_) => unreachable!(),
            };
            let (delta, done) = match api {
                "chat" => (
                    json!({"choices": [{"index": 0, "delta": {"content": text}}]}),
                    "[DONE]".to_owned(),
                ),
                "responses" => (
                    json!({"type": "response.output_text.delta", "delta": text}),
                    json!({"type": "response.completed", "response": {"status": "completed"}})
                        .to_string(),
                ),
                "messages" => (
                    json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": text}}),
                    json!({"type": "message_stop"}).to_string(),
                ),
                _ => unreachable!(),
            };
            let mut body = format!("data: {delta}\n\n");
            if !hold {
                body.push_str(&format!("data: {done}\n\n"));
            }
            ("200 OK", "text/event-stream", body, hold)
        }
    };
    let length = if hold {
        String::new()
    } else {
        format!("Content-Length: {}\r\n", body.len())
    };
    let headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\n{length}Connection: close\r\n\r\n"
    );
    stream.write_all(headers.as_bytes()).await?;
    stream.write_all(body.as_bytes()).await?;
    if hold {
        let mut byte = [0];
        let _ = stream.read(&mut byte).await?;
    }
    Ok(())
}
