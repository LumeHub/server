use std::collections::HashMap;
use std::sync::Arc;

use httpd::{Request, Response, Router};

use super::device::{GoogleDevice, LedStrip};
use super::request::{Intent, RequestInput, SmartHomeRequest};
use super::response::{
    ExecuteResponse, QueryResponse, ResponsePayload, SmartHomeResponse, SyncResponse,
};

use application::SceneRuntime;

use super::commands;

/// Dependencies of the Smart Home fulfillment endpoint.
#[derive(Clone)]
pub struct GoogleState {
    pub runtime: Arc<dyn SceneRuntime>,
    pub dispatcher: commands::CommandDispatcher,
}

pub async fn smarthome(st: GoogleState, http: Request) -> Response {
    let req: SmartHomeRequest = match http.json() {
        Ok(r) => r,
        Err(e) => return httpd::IntoResponse::into_response(e),
    };
    let runtime = &st.runtime;
    let command_dispatcher = &st.dispatcher;
    let response_payload = match req.inputs.first() {
        Some(input) => match input.intent {
            Intent::Sync => handle_sync(),
            Intent::Query => handle_query(input, runtime.as_ref()),
            Intent::Execute => handle_execute(input, runtime.as_ref(), command_dispatcher),
            Intent::Disconnect => ResponsePayload::Error(super::response::ErrorResponse {
                error_code: "Unsupported intent: DISCONNECT".to_string(),
            }),
        },
        None => ResponsePayload::Error(super::response::ErrorResponse {
            error_code: "No input found in request".to_string(),
        }),
    };

    Response::json(
        200,
        &SmartHomeResponse {
            request_id: req.request_id.clone(),
            payload: response_payload,
        },
    )
}

fn handle_sync() -> ResponsePayload {
    let device = LedStrip;
    let sync_response = SyncResponse {
        agent_user_id: "123".to_string(),
        devices: vec![device.sync()],
    };
    ResponsePayload::Sync(sync_response)
}

fn handle_query(input: &RequestInput, runtime: &dyn SceneRuntime) -> ResponsePayload {
    let device = LedStrip;
    let snap = runtime.snapshot();

    let devices_response: HashMap<String, serde_json::Value> = input
        .payload
        .as_ref()
        .and_then(|p| p.devices.as_ref())
        .map_or_else(HashMap::new, |devices_to_query| {
            devices_to_query
                .iter()
                .filter(|d| d.id == device.sync().id)
                .map(|d| (d.id.clone(), device.query(&snap)))
                .collect()
        });

    ResponsePayload::Query(QueryResponse {
        devices: devices_response,
    })
}

fn handle_execute(
    input: &RequestInput,
    runtime: &dyn SceneRuntime,
    command_dispatcher: &commands::CommandDispatcher,
) -> ResponsePayload {
    let command_responses = input
        .payload
        .as_ref()
        .and_then(|p| p.commands.as_ref())
        .map(|commands| {
            commands
                .iter()
                .flat_map(|cmd| command_dispatcher.process_command(cmd, runtime))
                .collect()
        })
        .unwrap_or_default();

    ResponsePayload::Execute(ExecuteResponse {
        commands: command_responses,
    })
}

pub fn routes(r: &mut Router, st: &GoogleState) {
    r.post("/smarthome", st.clone(), smarthome);
}
