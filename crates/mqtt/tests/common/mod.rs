use std::sync::{Arc, Mutex};

use application::{SceneRuntime, SceneSnapshot};
use domain::Rgb;
use mqtt::MqttConfig;
use mqtt::ctx::Ctx;
use mqtt::topics::Topics;
use mqtt::transport::MqttClient;
use store::Store;

pub struct MockRuntime {
    pub on: Mutex<bool>,
    pub brightness: Mutex<u8>,
    pub color: Mutex<Rgb>,
    pub reload_count: Mutex<usize>,
    pub halt_count: Mutex<usize>,
}

impl MockRuntime {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            on: Mutex::new(true),
            brightness: Mutex::new(255),
            color: Mutex::new(Rgb::BLACK),
            reload_count: Mutex::new(0),
            halt_count: Mutex::new(0),
        })
    }
}

impl SceneRuntime for MockRuntime {
    fn set_color(&self, c: Rgb) {
        *self.color.lock().unwrap() = c;
    }
    fn set_brightness(&self, b: u8) {
        *self.brightness.lock().unwrap() = b;
    }
    fn set_on_off(&self, v: bool) {
        *self.on.lock().unwrap() = v;
    }
    fn halt(&self) {
        *self.halt_count.lock().unwrap() += 1;
    }
    fn stop_effect(&self) {}
    fn reload_active(&self) {
        *self.reload_count.lock().unwrap() += 1;
    }
    fn snapshot(&self) -> SceneSnapshot {
        SceneSnapshot {
            on: *self.on.lock().unwrap(),
            brightness: *self.brightness.lock().unwrap(),
            color: *self.color.lock().unwrap(),
            active_effect: None,
            light_effect_end_unix_timestamp_sec: None,
        }
    }
}

pub async fn in_memory_store() -> Arc<Store> {
    Arc::new(Store::open_in_memory().await.unwrap())
}

/// A client whose connection is already gone.
/// Publish/subscribe calls fail silently — tests verify store/runtime state instead.
pub fn disconnected_client() -> MqttClient {
    MqttClient::disconnected()
}

pub fn payload(v: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&v).unwrap()
}

pub async fn make_ctx(runtime: Arc<MockRuntime>, store: Arc<Store>) -> Ctx {
    Ctx {
        client: disconnected_client(),
        topics: Topics::new("lumehub"),
        config: MqttConfig::default(),
        runtime: runtime as Arc<dyn SceneRuntime>,
        store,
        builtins: Arc::new(vec![]),
        active_effect: Arc::new(Mutex::new(None)),
    }
}
