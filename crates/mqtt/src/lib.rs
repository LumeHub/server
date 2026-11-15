mod client;
mod discovery;
mod types;

pub mod config;
pub mod ctx;
pub mod handlers;
pub mod publish;
pub mod topics;
pub mod transport;
pub mod wire;

pub use config::MqttConfig;

use std::sync::Arc;

use application::{SceneRuntime, StateEventBus};
use effects::BuiltinEffect;
use store::Store;

/// Run the MQTT bridge in the background for the life of the process,
/// reconnecting on any failure.
pub fn spawn(
    config: MqttConfig,
    runtime: Arc<dyn SceneRuntime>,
    store: Arc<Store>,
    event_bus: StateEventBus,
    builtins: Vec<BuiltinEffect>,
) {
    smol::spawn(client::run(config, runtime, store, event_bus, builtins)).detach();
}
