use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{IntoResponse, Method, Response};

/// A fully-read HTTP request. Header names are stored lower-case.
#[derive(Debug, Clone)]
pub struct Request {
    method: Method,
    path: String,
    query: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    params: Vec<(String, String)>,
}

/// Malformed client input; becomes a `400 Bad Request` with the message.
#[derive(Debug)]
pub struct BadRequest(pub String);

impl IntoResponse for BadRequest {
    fn into_response(self) -> Response {
        Response::text(400, self.0)
    }
}

impl Request {
    /// `target` is the request-target, path plus optional `?query`.
    pub fn new(method: Method, target: &str) -> Self {
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        Self {
            method,
            path: path.to_string(),
            query: query.to_string(),
            headers: Vec::new(),
            body: Vec::new(),
            params: Vec::new(),
        }
    }

    pub fn get(target: &str) -> Self {
        Self::new(Method::Get, target)
    }
    pub fn post(target: &str) -> Self {
        Self::new(Method::Post, target)
    }
    pub fn put(target: &str) -> Self {
        Self::new(Method::Put, target)
    }
    pub fn patch(target: &str) -> Self {
        Self::new(Method::Patch, target)
    }
    pub fn delete(target: &str) -> Self {
        Self::new(Method::Delete, target)
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers
            .push((name.to_ascii_lowercase(), value.to_string()));
        self
    }

    pub fn with_body(self, body: Vec<u8>, content_type: &str) -> Self {
        let mut req = self.with_header("content-type", content_type);
        req.body = body;
        req
    }

    pub fn with_json(self, value: &impl Serialize) -> Self {
        let body = serde_json::to_vec(value).expect("serializable test body");
        self.with_body(body, "application/json")
    }

    pub fn method(&self) -> Method {
        self.method
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Value of a `{name}` path segment; empty if the route has none.
    pub fn param(&self, name: &str) -> &str {
        self.params
            .iter()
            .find(|(n, _)| n == name)
            .map_or("", |(_, v)| v.as_str())
    }

    pub fn json<T: DeserializeOwned>(&self) -> Result<T, BadRequest> {
        serde_json::from_slice(&self.body)
            .map_err(|e| BadRequest(format!("invalid JSON body: {e}")))
    }

    pub fn form<T: DeserializeOwned>(&self) -> Result<T, BadRequest> {
        serde_urlencoded::from_bytes(&self.body)
            .map_err(|e| BadRequest(format!("invalid form body: {e}")))
    }

    pub fn query<T: DeserializeOwned>(&self) -> Result<T, BadRequest> {
        serde_urlencoded::from_str(&self.query)
            .map_err(|e| BadRequest(format!("invalid query: {e}")))
    }

    pub(crate) fn set_params(&mut self, params: Vec<(String, String)>) {
        self.params = params;
    }

    pub(crate) fn from_parts(
        method: Method,
        target: &str,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    ) -> Self {
        let mut req = Self::new(method, target);
        req.headers = headers;
        req.body = body;
        req
    }
}
