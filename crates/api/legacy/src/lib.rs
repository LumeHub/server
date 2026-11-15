mod color;
mod state;

use std::sync::Arc;

use application::SceneRuntime;

/// Endpoints of the original LumeHub app, kept for old clients.
pub fn routes(r: &mut httpd::Router, runtime: &Arc<dyn SceneRuntime>) {
    r.post("/led/getColor", runtime.clone(), color::led_get_color);
    r.post("/led/setColor", runtime.clone(), color::led_set_color);
    r.post("/led/state/get", runtime.clone(), state::led_state_get);
    r.post("/led/state/on", runtime.clone(), state::led_state_on);
    r.post("/led/state/off", runtime.clone(), state::led_state_off);
}
