use std::io;
use std::sync::Arc;
use std::time::Duration;

use smol::future::FutureExt;
use smol::io::{AsyncReadExt, AsyncWriteExt};
use smol::net::{TcpListener, TcpStream};
use smol::stream::StreamExt;

use crate::response::reason;
use crate::{Body, Method, Request, Response, Router};

const MAX_HEAD: usize = 16 * 1024;
const MAX_HEADERS: usize = 64;
/// Effect scripts are the largest bodies; this is far above any real one.
const MAX_BODY: usize = 8 * 1024 * 1024;
/// Idle keep-alive connections, and stalled requests, are closed after this.
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// Accept connections forever, serving each on its own task.
pub async fn serve(listener: TcpListener, router: Arc<Router>) -> io::Result<()> {
    loop {
        let stream = match listener.accept().await {
            Ok((stream, _)) => stream,
            Err(e) => {
                // e.g. EMFILE: back off instead of spinning or dying.
                eprintln!("[http] accept failed: {e}");
                smol::Timer::after(Duration::from_millis(100)).await;
                continue;
            }
        };
        let router = Arc::clone(&router);
        smol::spawn(async move {
            let _ = Connection::new(stream).run(&router).await;
        })
        .detach();
    }
}

struct Head {
    method: Method,
    target: String,
    keep_alive: bool,
    headers: Vec<(String, String)>,
    len: usize,
}

/// Why a request couldn't be read; each maps to a final error response.
enum Reject {
    Status(u16),
    /// Peer went away or idled out between requests: just close.
    Closed,
}

struct Connection {
    stream: TcpStream,
    buf: Vec<u8>,
}

impl Connection {
    fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            buf: Vec::new(),
        }
    }

    async fn run(mut self, router: &Router) -> io::Result<()> {
        loop {
            let (req, keep_alive) = match self.read_request().await? {
                Ok(r) => r,
                Err(Reject::Closed) => return Ok(()),
                Err(Reject::Status(status)) => {
                    return self.write(Response::empty(status), false).await;
                }
            };
            let resp = router.call(req).await;
            self.write(resp, keep_alive).await?;
            if !keep_alive {
                return Ok(());
            }
        }
    }

    async fn read_request(&mut self) -> io::Result<Result<(Request, bool), Reject>> {
        let head = match self.read_head().await? {
            Ok(h) => h,
            Err(r) => return Ok(Err(r)),
        };
        if header(&head.headers, "transfer-encoding").is_some() {
            return Ok(Err(Reject::Status(501)));
        }
        let content_length = match header(&head.headers, "content-length") {
            None => 0,
            Some(v) => match v.trim().parse::<usize>() {
                Ok(n) => n,
                Err(_) => return Ok(Err(Reject::Status(400))),
            },
        };
        if content_length > MAX_BODY {
            return Ok(Err(Reject::Status(413)));
        }
        if content_length > 0
            && header(&head.headers, "expect")
                .is_some_and(|v| v.eq_ignore_ascii_case("100-continue"))
        {
            self.stream
                .write_all(b"HTTP/1.1 100 Continue\r\n\r\n")
                .await?;
        }

        let end = head.len + content_length;
        while self.buf.len() < end {
            if !self.fill().await? {
                return Ok(Err(Reject::Closed));
            }
        }
        let body = self.buf[head.len..end].to_vec();
        // Keep any pipelined bytes of the next request.
        self.buf.drain(..end);

        let req = Request::from_parts(head.method, &head.target, head.headers, body);
        Ok(Ok((req, head.keep_alive)))
    }

    async fn read_head(&mut self) -> io::Result<Result<Head, Reject>> {
        loop {
            let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
            let mut parsed = httparse::Request::new(&mut headers);
            match parsed.parse(&self.buf) {
                Ok(httparse::Status::Complete(len)) => return Ok(Ok(to_head(&parsed, len))),
                Ok(httparse::Status::Partial) => {}
                Err(httparse::Error::TooManyHeaders) => return Ok(Err(Reject::Status(431))),
                Err(_) => return Ok(Err(Reject::Status(400))),
            }
            if self.buf.len() > MAX_HEAD {
                return Ok(Err(Reject::Status(431)));
            }
            let was_empty = self.buf.is_empty();
            if !self.fill().await? {
                // Idle/closed before a new request started: not an error.
                return Ok(Err(if was_empty {
                    Reject::Closed
                } else {
                    Reject::Status(408)
                }));
            }
        }
    }

    /// Read more bytes. `false` on EOF or idle timeout.
    async fn fill(&mut self) -> io::Result<bool> {
        let mut chunk = [0u8; 8192];
        let read = async { self.stream.read(&mut chunk).await.map(Some) };
        let timeout = async {
            smol::Timer::after(IDLE_TIMEOUT).await;
            Ok(None)
        };
        match read.or(timeout).await? {
            Some(0) | None => Ok(false),
            Some(n) => {
                self.buf.extend_from_slice(&chunk[..n]);
                Ok(true)
            }
        }
    }

    async fn write(&mut self, resp: Response, keep_alive: bool) -> io::Result<()> {
        let (status, headers, body) = resp.into_parts();
        let mut head = format!("HTTP/1.1 {status} {}\r\n", reason(status));
        for (name, value) in headers.iter().filter(|(n, _)| !is_framing_header(n)) {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        if !keep_alive {
            head.push_str("connection: close\r\n");
        }
        match body {
            Body::Bytes(bytes) => {
                head.push_str(&format!("content-length: {}\r\n\r\n", bytes.len()));
                let mut out = head.into_bytes();
                out.extend_from_slice(&bytes);
                self.stream.write_all(&out).await
            }
            Body::Stream(mut chunks) => {
                head.push_str("transfer-encoding: chunked\r\n\r\n");
                self.stream.write_all(head.as_bytes()).await?;
                while let Some(chunk) = chunks.next().await {
                    if chunk.is_empty() {
                        continue; // an empty chunk would end the body
                    }
                    let mut out = format!("{:x}\r\n", chunk.len()).into_bytes();
                    out.extend_from_slice(&chunk);
                    out.extend_from_slice(b"\r\n");
                    self.stream.write_all(&out).await?;
                }
                self.stream.write_all(b"0\r\n\r\n").await
            }
        }
    }
}

fn to_head(parsed: &httparse::Request, len: usize) -> Head {
    let headers: Vec<(String, String)> = parsed
        .headers
        .iter()
        .map(|h| {
            (
                h.name.to_ascii_lowercase(),
                String::from_utf8_lossy(h.value).into_owned(),
            )
        })
        .collect();
    let connection = header(&headers, "connection").map(str::to_ascii_lowercase);
    let keep_alive = match parsed.version {
        Some(1) => connection.as_deref() != Some("close"),
        _ => connection.as_deref() == Some("keep-alive"),
    };
    Head {
        method: Method::parse(parsed.method.unwrap_or("")),
        target: parsed.path.unwrap_or("/").to_string(),
        keep_alive,
        headers,
        len,
    }
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.as_str())
}

/// Framing is decided here, never by handlers.
fn is_framing_header(name: &str) -> bool {
    matches!(name, "content-length" | "transfer-encoding" | "connection")
}
