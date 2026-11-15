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
            &[api::scenes::routes],
        )
    }};
}

#[apply(test!)]
async fn crud() {
    let svc = svc!(make_store().await);

    // create
    let resp = svc
        .call(Request::post("/scenes").with_json(&json!({"name": "Night"})))
        .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = resp.json_body().unwrap();
    assert_eq!(body["name"], "Night");
    let id = body["id"].as_str().unwrap().to_string();

    // get with empty layers
    let resp = svc.call(Request::get(&format!("/scenes/{id}"))).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json_body().unwrap();
    assert_eq!(body["layers"].as_array().unwrap().len(), 0);

    // list
    let resp = svc.call(Request::get("/scenes")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Vec<Value> = resp.json_body().unwrap();
    assert_eq!(body.len(), 1);

    // rename
    let resp = svc
        .call(Request::put(&format!("/scenes/{id}")).with_json(&json!({"name": "Day"})))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json_body().unwrap();
    assert_eq!(body["name"], "Day");

    // delete
    let resp = svc.call(Request::delete(&format!("/scenes/{id}"))).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = svc.call(Request::get(&format!("/scenes/{id}"))).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
