use httpd::{Request, Response, Router};

use crate::ApiState;

/// Server-sent events: one `data:` line per device state change.
pub async fn events(st: ApiState, _req: Request) -> Response {
    let rx = st.events.subscribe();
    let chunks = smol::stream::unfold(rx, |mut rx| async move {
        let snapshot = rx.recv().await?;
        let json = serde_json::to_string(&snapshot).unwrap_or_default();
        Some((format!("data: {json}\n\n").into_bytes(), rx))
    });
    Response::event_stream(chunks)
}

pub fn routes(r: &mut Router, st: &ApiState) {
    r.get("/events", st.clone(), events);
}
