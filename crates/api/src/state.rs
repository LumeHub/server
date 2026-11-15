use std::sync::Arc;

use application::{SceneRuntime, StateEventBus};
use effects::BuiltinEffect;
use store::Store;

/// Everything the REST handlers need; cloned (cheaply) into each request.
#[derive(Clone)]
pub struct ApiState {
    pub store: Arc<Store>,
    pub runtime: Arc<dyn SceneRuntime>,
    pub builtins: Arc<Vec<BuiltinEffect>>,
    pub strip_len: usize,
    pub events: StateEventBus,
}

/// Register every REST route.
pub fn routes(r: &mut httpd::Router, st: &ApiState) {
    crate::device::routes(r, st);
    crate::effects::routes(r, st);
    crate::events::routes(r, st);
    crate::zones::routes(r, st);
    crate::active_scene::routes(r, st);
    crate::scenes::routes(r, st);
}
