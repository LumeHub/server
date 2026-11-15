use actix_web::{HttpResponse, get, web};
use application::StateEventBus;
use futures_util::stream;

#[get("/events")]
pub async fn events(bus: web::Data<StateEventBus>) -> HttpResponse {
    let rx = bus.subscribe();

    let stream = stream::unfold(rx, |mut rx| async move {
        let snapshot = rx.recv().await?;
        let json = serde_json::to_string(&snapshot).unwrap_or_default();
        let bytes = web::Bytes::from(format!("data: {json}\n\n"));
        Some((Ok::<_, actix_web::Error>(bytes), rx))
    });

    HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(stream)
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(events);
}
