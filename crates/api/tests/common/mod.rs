use std::sync::Arc;

use api::ApiState;
use application::{SceneRuntime, SceneSnapshot, StateEventBus};
use domain::Rgb;
use httpd::Router;
use store::Store;

pub struct MockRuntime;

impl SceneRuntime for MockRuntime {
    fn set_color(&self, _: Rgb) {}
    fn set_brightness(&self, _: u8) {}
    fn set_on_off(&self, _: bool) {}
    fn halt(&self) {}
    fn stop_effect(&self) {}
    fn reload_active(&self) {}
    fn snapshot(&self) -> SceneSnapshot {
        SceneSnapshot {
            on: true,
            brightness: 255,
            color: Rgb::BLACK,
            active_effect: None,
            light_effect_end_unix_timestamp_sec: None,
        }
    }
}

pub async fn make_store() -> Arc<Store> {
    Arc::new(Store::open_in_memory().await.unwrap())
}

pub fn mock_runtime() -> Arc<dyn SceneRuntime> {
    Arc::new(MockRuntime)
}

/// Strip length the handlers see in tests.
pub const TEST_STRIP_LEN: usize = 100;

pub fn state(store: Arc<Store>, runtime: Arc<dyn SceneRuntime>) -> ApiState {
    ApiState {
        store,
        runtime,
        builtins: Arc::new(effects::load_builtins()),
        strip_len: TEST_STRIP_LEN,
        events: StateEventBus::new(),
    }
}

pub fn router(st: &ApiState, register: &[fn(&mut Router, &ApiState)]) -> Router {
    let mut r = Router::new();
    for f in register {
        f(&mut r, st);
    }
    r
}
