mod common;

use common::{make_store, mock_runtime};
use httpd::{Request, StatusCode};
use macro_rules_attribute::apply;
use serde_json::{Value, json};
use smol_macros::test;

macro_rules! svc {
    ($store:expr) => {{
        common::router(
            &common::state($store, mock_runtime()),
            &[api::device::routes],
        )
    }};
}

#[apply(test!)]
async fn get_state() {
    let svc = svc!(make_store().await);

    let resp = svc.call(Request::get("/device/state")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json_body().unwrap();
    assert!(body["on"].is_boolean());
    assert!(body["brightness"].is_number());
    assert!(body["color"].is_object());
}

#[apply(test!)]
async fn patch_state() {
    let svc = svc!(make_store().await);

    let resp = svc
        .call(Request::patch("/device/state").with_json(&json!({"on": false, "brightness": 100})))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json_body().unwrap();
    assert!(body["on"].is_boolean());
}
