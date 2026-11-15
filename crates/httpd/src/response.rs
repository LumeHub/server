use std::pin::Pin;

use serde::Serialize;
use smol::stream::Stream;

pub type ChunkStream = Pin<Box<dyn Stream<Item = Vec<u8>> + Send>>;

pub enum Body {
    Bytes(Vec<u8>),
    /// Sent with chunked transfer encoding until the stream ends.
    Stream(ChunkStream),
}

pub struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Body,
}

/// Status codes used by the handlers and their tests.
pub struct StatusCode;

impl StatusCode {
    pub const OK: u16 = 200;
    pub const CREATED: u16 = 201;
    pub const NO_CONTENT: u16 = 204;
    pub const FOUND: u16 = 302;
    pub const BAD_REQUEST: u16 = 400;
    pub const FORBIDDEN: u16 = 403;
    pub const NOT_FOUND: u16 = 404;
    pub const INTERNAL_SERVER_ERROR: u16 = 500;
}

impl Response {
    pub fn new(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Body::Bytes(Vec::new()),
        }
    }

    pub fn empty(status: u16) -> Self {
        Self::new(status)
    }

    pub fn text(status: u16, text: impl Into<String>) -> Self {
        Self::new(status).with_body(text.into().into_bytes(), "text/plain; charset=utf-8")
    }

    pub fn json(status: u16, value: &impl Serialize) -> Self {
        match serde_json::to_vec(value) {
            Ok(body) => Self::new(status).with_body(body, "application/json"),
            Err(e) => Self::text(500, format!("failed to serialize response: {e}")),
        }
    }

    pub fn redirect(location: &str) -> Self {
        Self::new(StatusCode::FOUND).with_header("location", location)
    }

    /// Server-sent events: each item is written as-is, flushed immediately.
    pub fn event_stream(chunks: impl Stream<Item = Vec<u8>> + Send + 'static) -> Self {
        Self {
            status: 200,
            headers: vec![
                ("content-type".into(), "text/event-stream".into()),
                ("cache-control".into(), "no-cache".into()),
                ("x-accel-buffering".into(), "no".into()),
            ],
            body: Body::Stream(Box::pin(chunks)),
        }
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers
            .push((name.to_ascii_lowercase(), value.to_string()));
        self
    }

    pub fn with_body(mut self, body: Vec<u8>, content_type: &str) -> Self {
        self.body = Body::Bytes(body);
        self.with_header("content-type", content_type)
    }

    pub fn status(&self) -> u16 {
        self.status
    }

    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Buffered body bytes; empty for streaming responses.
    pub fn body_bytes(&self) -> &[u8] {
        match &self.body {
            Body::Bytes(b) => b,
            Body::Stream(_) => &[],
        }
    }

    /// Parse the buffered body as JSON (for tests and clients).
    pub fn json_body<T: serde::de::DeserializeOwned>(&self) -> serde_json::Result<T> {
        serde_json::from_slice(self.body_bytes())
    }

    pub(crate) fn into_parts(self) -> (u16, Vec<(String, String)>, Body) {
        (self.status, self.headers, self.body)
    }
}

pub trait IntoResponse {
    fn into_response(self) -> Response;
}

impl IntoResponse for Response {
    fn into_response(self) -> Response {
        self
    }
}

impl<T: IntoResponse, E: IntoResponse> IntoResponse for Result<T, E> {
    fn into_response(self) -> Response {
        match self {
            Ok(v) => v.into_response(),
            Err(e) => e.into_response(),
        }
    }
}

pub(crate) fn reason(status: u16) -> &'static str {
    match status {
        100 => "Continue",
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        302 => "Found",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        _ => "",
    }
}
