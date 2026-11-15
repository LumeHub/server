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
            &[api::active_scene::routes, api::scenes::routes],
        )
    }};
}

#[apply(test!)]
async fn starts_empty() {
    let svc = svc!(make_store().await);

    let resp = svc.call(Request::get("/scenes/active")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Vec<Value> = resp.json_body().unwrap();
    assert!(body.is_empty());
}

#[apply(test!)]
async fn layer_lifecycle() {
    let svc = svc!(make_store().await);

    // add
    let resp = svc
        .call(Request::post("/scenes/active/layers").with_json(&json!({
            "effect_id": "builtin:rainbow",
            "zone_id": "all",
            "blend_mode": "override"
        })))
        .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = resp.json_body().unwrap();
    assert_eq!(body["effect_id"], "builtin:rainbow");
    assert_eq!(body["enabled"], true);
    let layer_id = body["id"].as_str().unwrap().to_string();

    // patch
    let resp = svc
        .call(
            Request::patch(&format!("/scenes/active/layers/{layer_id}"))
                .with_json(&json!({"enabled": false})),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json_body().unwrap();
    assert_eq!(body["enabled"], false);

    // still 1 layer in GET /scenes/active
    let resp = svc.call(Request::get("/scenes/active")).await;
    let body: Vec<Value> = resp.json_body().unwrap();
    assert_eq!(body.len(), 1);

    // remove
    let resp = svc
        .call(Request::delete(&format!(
            "/scenes/active/layers/{layer_id}"
        )))
        .await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = svc.call(Request::get("/scenes/active")).await;
    let body: Vec<Value> = resp.json_body().unwrap();
    assert!(body.is_empty());
}

#[apply(test!)]
async fn clear() {
    let svc = svc!(make_store().await);

    svc.call(Request::post("/scenes/active/layers").with_json(
        &json!({"effect_id": "builtin:aurora", "zone_id": "all", "blend_mode": "override"}),
    ))
    .await;

    let resp = svc.call(Request::delete("/scenes/active")).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = svc.call(Request::get("/scenes/active")).await;
    let body: Vec<Value> = resp.json_body().unwrap();
    assert!(body.is_empty());
}

#[apply(test!)]
async fn save_and_load() {
    let svc = svc!(make_store().await);

    svc.call(Request::post("/scenes/active/layers").with_json(
        &json!({"effect_id": "builtin:rainbow", "zone_id": "all", "blend_mode": "override"}),
    ))
    .await;

    // save
    let resp = svc
        .call(Request::post("/scenes/active/save").with_json(&json!({"name": "Saved Rainbow"})))
        .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = resp.json_body().unwrap();
    let scene_id = body["id"].as_str().unwrap().to_string();

    // clear, then load
    svc.call(Request::delete("/scenes/active")).await;

    let resp = svc
        .call(Request::post(&format!("/scenes/active/load/{scene_id}")))
        .await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = svc.call(Request::get("/scenes/active")).await;
    let body: Vec<Value> = resp.json_body().unwrap();
    assert_eq!(body.len(), 1);
    assert_eq!(body[0]["effect_id"], "builtin:rainbow");
}

#[apply(test!)]
async fn load_nonexistent_returns_404() {
    let svc = svc!(make_store().await);

    let resp = svc
        .call(Request::post("/scenes/active/load/no-such-scene"))
        .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
