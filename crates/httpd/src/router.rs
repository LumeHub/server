use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::{IntoResponse, Method, Request, Response};

type BoxFuture = Pin<Box<dyn Future<Output = Response> + Send>>;
type Handler = Arc<dyn Fn(Request) -> BoxFuture + Send + Sync>;

enum Segment {
    Literal(String),
    Param(String),
}

struct Route {
    method: Method,
    segments: Vec<Segment>,
    handler: Handler,
}

/// Method + path router. A route's `{name}` segments match any single path
/// segment. When several routes match, the one with the most literal
/// segments wins, so `/scenes/active` beats `/scenes/{id}` whatever the
/// registration order; equal candidates go to the earliest registered.
#[derive(Default)]
pub struct Router {
    routes: Vec<Route>,
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `handler`, which receives a clone of `state` per request.
    pub fn route<S, F, Fut, R>(&mut self, method: Method, pattern: &str, state: S, handler: F)
    where
        S: Clone + Send + Sync + 'static,
        F: Fn(S, Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse,
    {
        let handler: Handler = Arc::new(move |req| {
            let fut = handler(state.clone(), req);
            Box::pin(async move { fut.await.into_response() })
        });
        self.routes.push(Route {
            method,
            segments: parse_pattern(pattern),
            handler,
        });
    }

    pub fn get<S, F, Fut, R>(&mut self, pattern: &str, state: S, handler: F)
    where
        S: Clone + Send + Sync + 'static,
        F: Fn(S, Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse,
    {
        self.route(Method::Get, pattern, state, handler);
    }

    pub fn post<S, F, Fut, R>(&mut self, pattern: &str, state: S, handler: F)
    where
        S: Clone + Send + Sync + 'static,
        F: Fn(S, Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse,
    {
        self.route(Method::Post, pattern, state, handler);
    }

    pub fn put<S, F, Fut, R>(&mut self, pattern: &str, state: S, handler: F)
    where
        S: Clone + Send + Sync + 'static,
        F: Fn(S, Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse,
    {
        self.route(Method::Put, pattern, state, handler);
    }

    pub fn patch<S, F, Fut, R>(&mut self, pattern: &str, state: S, handler: F)
    where
        S: Clone + Send + Sync + 'static,
        F: Fn(S, Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse,
    {
        self.route(Method::Patch, pattern, state, handler);
    }

    pub fn delete<S, F, Fut, R>(&mut self, pattern: &str, state: S, handler: F)
    where
        S: Clone + Send + Sync + 'static,
        F: Fn(S, Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse,
    {
        self.route(Method::Delete, pattern, state, handler);
    }

    /// Dispatch a request in-process; also what the TCP server calls.
    pub async fn call(&self, mut req: Request) -> Response {
        let parts: Vec<&str> = req.path().split('/').filter(|s| !s.is_empty()).collect();
        let best = self
            .routes
            .iter()
            .filter(|r| r.method == req.method())
            .filter_map(|r| match_route(r, &parts).map(|params| (r, params)))
            .min_by_key(|(r, _)| std::cmp::Reverse(literal_count(r)));
        match best {
            Some((route, params)) => {
                req.set_params(params);
                (route.handler)(req).await
            }
            None => Response::empty(404),
        }
    }
}

fn parse_pattern(pattern: &str) -> Vec<Segment> {
    pattern
        .split('/')
        .filter(|s| !s.is_empty())
        .map(
            |s| match s.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                Some(name) => Segment::Param(name.to_string()),
                None => Segment::Literal(s.to_string()),
            },
        )
        .collect()
}

fn literal_count(route: &Route) -> usize {
    route
        .segments
        .iter()
        .filter(|s| matches!(s, Segment::Literal(_)))
        .count()
}

fn match_route(route: &Route, parts: &[&str]) -> Option<Vec<(String, String)>> {
    if route.segments.len() != parts.len() {
        return None;
    }
    let mut params = Vec::new();
    for (seg, part) in route.segments.iter().zip(parts) {
        match seg {
            Segment::Literal(lit) if lit == part => {}
            Segment::Literal(_) => return None,
            Segment::Param(name) => params.push((name.clone(), percent_decode(part))),
        }
    }
    Some(params)
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(hex) = bytes.get(i + 1..i + 3)
            && let Ok(v) = u8::from_str_radix(std::str::from_utf8(hex).unwrap_or("zz"), 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
