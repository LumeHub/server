use std::sync::{Arc, Mutex};

use application::{SceneRuntime, SceneSnapshot};
use domain::Rgb;
use httpd::{Request, Router};
use serde_json::{Value, json};

#[derive(Default)]
struct Runtime {
    on: Mutex<bool>,
    color: Mutex<Option<Rgb>>,
}

impl SceneRuntime for Runtime {
    fn set_color(&self, c: Rgb) {
        *self.color.lock().unwrap() = Some(c);
    }
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
            color: self.color.lock().unwrap().unwrap_or(Rgb::BLACK),
            active_effect: None,
            light_effect_end_unix_timestamp_sec: None,
        }
    }
}

fn router() -> Router {
    let runtime: Arc<dyn SceneRuntime> = Arc::new(Runtime::default());
    let mut r = Router::new();
    api_legacy::routes(&mut r, &runtime);
    r
}

fn post(r: &Router, path: &str, body: Option<Value>) -> (u16, Value) {
    let req = match body {
        Some(b) => Request::post(path).with_json(&b),
        None => Request::post(path),
    };
    let resp = smol::block_on(r.call(req));
    (resp.status(), resp.json_body().unwrap_or(Value::Null))
}

#[test]
fn set_then_get_color() {
    let r = router();
    let (status, body) = post(
        &r,
        "/led/setColor",
        Some(json!({"red": 1, "green": 2, "blue": 3})),
    );
    assert_eq!(status, 200);
    assert_eq!(body, json!({"red": 1, "green": 2, "blue": 3}));
    let (_, body) = post(&r, "/led/getColor", None);
    assert_eq!(body["green"], 2);
}

#[test]
fn set_color_rejects_bad_json() {
    let r = router();
    let (status, _) = post(&r, "/led/setColor", Some(json!({"red": 1})));
    assert_eq!(status, 400);
}

#[test]
fn state_on_off_and_get() {
    let r = router();
    assert_eq!(post(&r, "/led/state/on", None).1, json!({"ledState": true}));
    assert_eq!(
        post(&r, "/led/state/get", None).1,
        json!({"ledState": true})
    );
    assert_eq!(
        post(&r, "/led/state/off", None).1,
        json!({"ledState": false})
    );
}
