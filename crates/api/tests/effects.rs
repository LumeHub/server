mod common;

use common::{make_store, mock_runtime};
use httpd::{Request, StatusCode};
use macro_rules_attribute::apply;
use serde_json::{Value, json};
use smol_macros::test;

macro_rules! svc {
    ($store:expr, $rt:expr) => {{ common::router(&common::state($store, $rt), &[api::effects::routes]) }};
}

#[apply(test!)]
async fn list_includes_builtins() {
    let svc = svc!(make_store().await, mock_runtime());

    let resp = svc.call(Request::get("/effects")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Vec<Value> = resp.json_body().unwrap();
    assert!(body.iter().any(|e| e["builtin"] == true));
}

#[apply(test!)]
async fn create_and_get() {
    let svc = svc!(make_store().await, mock_runtime());

    let resp = svc
        .call(
            Request::post("/effects")
                .with_json(&json!({"name": "My Effect", "script": "rgb(255,0,0)", "params": []})),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = resp.json_body().unwrap();
    assert_eq!(body["name"], "My Effect");
    assert_eq!(body["builtin"], false);
    let id = body["id"].as_str().unwrap().to_string();

    let resp = svc.call(Request::get(&format!("/effects/{id}"))).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json_body().unwrap();
    assert_eq!(body["id"], id);
}

#[apply(test!)]
async fn update_user_effect() {
    let svc = svc!(make_store().await, mock_runtime());

    let resp = svc
        .call(Request::post("/effects").with_json(&json!({"name": "Old", "script": "rgb(0,0,0)"})))
        .await;
    let body: Value = resp.json_body().unwrap();
    let id = body["id"].as_str().unwrap().to_string();

    let resp = svc
        .call(
            Request::put(&format!("/effects/{id}"))
                .with_json(&json!({"name": "New", "script": "rgb(255,255,255)"})),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json_body().unwrap();
    assert_eq!(body["name"], "New");
}

#[apply(test!)]
async fn delete_user_effect() {
    let svc = svc!(make_store().await, mock_runtime());

    let resp = svc
        .call(Request::post("/effects").with_json(&json!({"name": "Temp", "script": "rgb(0,0,0)"})))
        .await;
    let body: Value = resp.json_body().unwrap();
    let id = body["id"].as_str().unwrap().to_string();

    let del = svc.call(Request::delete(&format!("/effects/{id}"))).await;
    assert_eq!(del.status(), StatusCode::NO_CONTENT);

    let get = svc.call(Request::get(&format!("/effects/{id}"))).await;
    assert_eq!(get.status(), StatusCode::NOT_FOUND);
}

#[apply(test!)]
async fn builtin_is_immutable() {
    let svc = svc!(make_store().await, mock_runtime());

    let put = svc
        .call(
            Request::put("/effects/builtin:rainbow")
                .with_json(&json!({"name": "Hacked", "script": "rgb(0,0,0)"})),
        )
        .await;
    assert_eq!(put.status(), StatusCode::FORBIDDEN);

    let del = svc.call(Request::delete("/effects/builtin:rainbow")).await;
    assert_eq!(del.status(), StatusCode::FORBIDDEN);
}

#[apply(test!)]
async fn get_nonexistent_returns_404() {
    let svc = svc!(make_store().await, mock_runtime());

    let resp = svc.call(Request::get("/effects/does-not-exist")).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
