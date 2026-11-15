use std::sync::{Arc, Mutex};

use api_google::GoogleState;
use api_google::commands::CommandDispatcher;
use application::{SceneRuntime, SceneSnapshot};
use domain::Rgb;
use httpd::{Request, Response, Router, StatusCode};
use serde_json::{Value, json};

#[derive(Default)]
struct Runtime {
    on: Mutex<bool>,
}

impl SceneRuntime for Runtime {
    fn set_color(&self, _: Rgb) {}
    fn set_brightness(&self, _: u8) {}
    fn set_on_off(&self, v: bool) {
        *self.on.lock().unwrap() = v;
    }
    fn halt(&self) {}
    fn stop_effect(&self) {}
    fn reload_active(&self) {}
    fn snapshot(&self) -> SceneSnapshot {
        SceneSnapshot {
            on: *self.on.lock().unwrap(),
            brightness: 255,
            color: Rgb::BLACK,
            active_effect: None,
            light_effect_end_unix_timestamp_sec: None,
        }
    }
}

fn router(runtime: Arc<Runtime>) -> Router {
    let mut r = Router::new();
    let st = GoogleState {
        runtime,
        dispatcher: CommandDispatcher::new(),
    };
    api_google::routes(&mut r, &st);
    r
}

fn call(r: &Router, req: Request) -> Response {
    smol::block_on(r.call(req))
}

#[test]
fn authorize_redirects_back_with_code_and_state() {
    let r = router(Arc::default());
    let resp = call(
        &r,
        Request::get(
            "/oauth/authorize?redirect_uri=https%3A%2F%2Fexample.com%2Fcb&state=xyz&client_id=g",
        ),
    );
    assert_eq!(resp.status(), StatusCode::FOUND);
    assert_eq!(
        resp.header("location"),
        Some("https://example.com/cb?code=fake-auth-code&state=xyz")
    );
}

#[test]
fn authorize_without_required_query_is_bad_request() {
    let r = router(Arc::default());
    assert_eq!(call(&r, Request::get("/oauth/authorize")).status(), 400);
}

fn form(body: &str) -> Request {
    Request::post("/oauth/token").with_body(
        body.as_bytes().to_vec(),
        "application/x-www-form-urlencoded",
    )
}

#[test]
fn token_exchanges_code_and_refreshes() {
    let r = router(Arc::default());
    let resp = call(&r, form("grant_type=authorization_code&client_id=g&code=c"));
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json_body().unwrap();
    assert_eq!(body["access_token"], "generated-access-token");
    assert_eq!(body["token_type"], "Bearer");

    let resp = call(
        &r,
        form("grant_type=refresh_token&client_id=g&refresh_token=r1"),
    );
    let body: Value = resp.json_body().unwrap();
    assert_eq!(body["refresh_token"], "r1");

    let resp = call(&r, form("grant_type=refresh_token&client_id=g"));
    assert_eq!(resp.status(), 400);
    let resp = call(&r, form("grant_type=password&client_id=g"));
    assert_eq!(resp.status(), 400);
}

#[test]
fn smarthome_sync_echoes_request_id() {
    let r = router(Arc::default());
    let resp = call(
        &r,
        Request::post("/smarthome")
            .with_json(&json!({"requestId": "r1", "inputs": [{"intent": "action.devices.SYNC"}]})),
    );
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json_body().unwrap();
    assert_eq!(body["requestId"], "r1");
    assert!(
        body["payload"]["devices"]
            .as_array()
            .is_some_and(|d| !d.is_empty())
    );
}

#[test]
fn smarthome_execute_drives_the_runtime() {
    let runtime = Arc::new(Runtime::default());
    let r = router(runtime.clone());
    let sync: Value = call(
        &r,
        Request::post("/smarthome")
            .with_json(&json!({"requestId": "s", "inputs": [{"intent": "action.devices.SYNC"}]})),
    )
    .json_body()
    .unwrap();
    let device_id = sync["payload"]["devices"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = call(
        &r,
        Request::post("/smarthome").with_json(&json!({
            "requestId": "e",
            "inputs": [{
                "intent": "action.devices.EXECUTE",
                "payload": {"commands": [{
                    "devices": [{"id": device_id}],
                    "execution": [{"command": "action.devices.commands.OnOff", "params": {"on": true}}]
                }]}
            }]
        })),
    );
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(*runtime.on.lock().unwrap());
}

#[test]
fn smarthome_rejects_malformed_json() {
    let r = router(Arc::default());
    let resp = call(
        &r,
        Request::post("/smarthome").with_body(b"{".to_vec(), "application/json"),
    );
    assert_eq!(resp.status(), 400);
}
