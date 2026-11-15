use application::SceneSnapshot;
use domain::Rgb;
use httpd::{Request, Response, Router};
use serde::{Deserialize, Serialize};

use crate::ApiState;
use crate::error::ApiResult;

#[derive(Serialize)]
struct DeviceStateResponse {
    on: bool,
    brightness: u8,
    color: Rgb,
    active_effect: Option<String>,
}

impl From<SceneSnapshot> for DeviceStateResponse {
    fn from(s: SceneSnapshot) -> Self {
        Self {
            on: s.on,
            brightness: s.brightness,
            color: s.color,
            active_effect: s.active_effect,
        }
    }
}

pub async fn get_state(st: ApiState, _req: Request) -> Response {
    Response::json(200, &DeviceStateResponse::from(st.runtime.snapshot()))
}

#[derive(Deserialize)]
struct PatchStateRequest {
    on: Option<bool>,
    brightness: Option<u8>,
    color: Option<Rgb>,
}

pub async fn patch_state(st: ApiState, req: Request) -> ApiResult {
    let body: PatchStateRequest = req.json()?;
    if let Some(on) = body.on {
        st.runtime.set_on_off(on);
    }
    if let Some(brightness) = body.brightness {
        st.runtime.set_brightness(brightness);
    }
    if let Some(color) = body.color {
        st.runtime.set_color(color);
    }
    Ok(Response::json(
        200,
        &DeviceStateResponse::from(st.runtime.snapshot()),
    ))
}

pub fn routes(r: &mut Router, st: &ApiState) {
    r.get("/device/state", st.clone(), get_state);
    r.patch("/device/state", st.clone(), patch_state);
}
