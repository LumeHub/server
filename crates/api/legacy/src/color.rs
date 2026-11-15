use std::sync::Arc;

use application::SceneRuntime;
use domain::Rgb;
use httpd::{Request, Response};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct SetColorRequest {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Serialize)]
pub struct LedColorResponse {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl From<Rgb> for LedColorResponse {
    fn from(rgb: Rgb) -> Self {
        Self {
            red: rgb.r,
            green: rgb.g,
            blue: rgb.b,
        }
    }
}

pub async fn led_get_color(runtime: Arc<dyn SceneRuntime>, _req: Request) -> Response {
    Response::json(200, &LedColorResponse::from(runtime.snapshot().color))
}

pub async fn led_set_color(runtime: Arc<dyn SceneRuntime>, http: Request) -> Response {
    let req: SetColorRequest = match serde_json::from_slice(http.body()) {
        Ok(r) => r,
        Err(e) => return Response::text(400, e.to_string()),
    };
    let color = Rgb {
        r: req.red,
        g: req.green,
        b: req.blue,
    };
    runtime.set_color(color);
    Response::json(200, &LedColorResponse::from(runtime.snapshot().color))
}
