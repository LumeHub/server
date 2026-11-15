use std::sync::Arc;

use application::SceneRuntime;
use httpd::{Request, Response};
use serde::Serialize;

#[derive(Serialize)]
pub struct LedStateResponse {
    #[serde(rename = "ledState")]
    pub led_state: bool,
}

pub async fn led_state_get(runtime: Arc<dyn SceneRuntime>, _req: Request) -> Response {
    Response::json(
        200,
        &LedStateResponse {
            led_state: runtime.snapshot().on,
        },
    )
}

pub async fn led_state_on(runtime: Arc<dyn SceneRuntime>, _req: Request) -> Response {
    runtime.set_on_off(true);
    Response::json(
        200,
        &LedStateResponse {
            led_state: runtime.snapshot().on,
        },
    )
}

pub async fn led_state_off(runtime: Arc<dyn SceneRuntime>, _req: Request) -> Response {
    runtime.set_on_off(false);
    Response::json(
        200,
        &LedStateResponse {
            led_state: runtime.snapshot().on,
        },
    )
}
