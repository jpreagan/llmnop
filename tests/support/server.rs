use serde_json::{Value, json};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

pub enum Reply {
    Text(String),
    Failure(String),
    Hold,
}

type Received = Arc<(Mutex<usize>, Condvar)>;

pub struct Server {
    url: String,
    received: Received,
}

impl Server {
    pub fn new(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1", listener.local_addr().unwrap());
        let received = Arc::new((Mutex::new(0), Condvar::new()));
        let counter = Arc::clone(&received);
        thread::spawn(move || {
            let mut replies = replies.into_iter();
            for stream in listener.incoming() {
                let Ok(stream) = stream else { break };
                let reply = replies.next();
                let counter = Arc::clone(&counter);
                thread::spawn(move || respond(stream, reply, &counter));
            }
        });
        Self { url, received }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn received(&self) -> usize {
        *self.received.0.lock().unwrap()
    }

    pub fn wait_for_requests(&self, count: usize, timeout: Duration) -> bool {
        let (received, changed) = &*self.received;
        let (received, _) = changed
            .wait_timeout_while(received.lock().unwrap(), timeout, |n| *n < count)
            .unwrap();
        *received >= count
    }
}

fn read_request(stream: &TcpStream) -> io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let mut length = 0;
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        if line == "\r\n" {
            break;
        }
        if let Some((_, value)) = line
            .split_once(':')
            .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        {
            length = value.trim().parse().map_err(io::Error::other)?;
        }
    }
    reader.read_exact(&mut vec![0; length])
}

fn respond(mut stream: TcpStream, reply: Option<Reply>, received: &Received) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    read_request(&stream)?;
    *received.0.lock().unwrap() += 1;
    received.1.notify_all();

    let hold = matches!(reply, Some(Reply::Hold));
    let (status, content_type, body) = match reply {
        Some(Reply::Text(text)) => (
            "200 OK",
            "text/event-stream",
            format!("data: {}\n\ndata: [DONE]\n\n", delta(&text)),
        ),
        Some(Reply::Hold) => (
            "200 OK",
            "text/event-stream",
            format!("data: {}\n\n", delta("hello")),
        ),
        Some(Reply::Failure(message)) => ("400 Bad Request", "text/plain", message),
        None => (
            "500 Internal Server Error",
            "text/plain",
            "unexpected request".to_owned(),
        ),
    };
    let length = if hold {
        String::new()
    } else {
        format!("Content-Length: {}\r\n", body.len())
    };
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\n{length}Connection: close\r\n\r\n{body}"
    )?;
    if hold {
        stream.set_read_timeout(None)?;
        let _ = stream.read(&mut [0])?;
    }
    Ok(())
}

fn delta(text: &str) -> Value {
    json!({"choices": [{"index": 0, "delta": {"content": text}}]})
}
