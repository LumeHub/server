//! Minimal HTTP/1.1 server and router on smol, sized for LumeHub's APIs:
//! JSON/form bodies, path params, keep-alive and chunked streaming (SSE).
//! No TLS, no request chunked encoding, no HTTP/2: the server sits on a LAN
//! behind clients that send `Content-Length`.

mod method;
mod request;
mod response;
mod router;
mod server;

pub use method::Method;
pub use request::{BadRequest, Request};
pub use response::{Body, IntoResponse, Response, StatusCode};
pub use router::Router;
pub use server::serve;
