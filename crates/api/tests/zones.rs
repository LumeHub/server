mod common;

use common::{make_store, mock_runtime};
use httpd::{Request, StatusCode};
use macro_rules_attribute::apply;
use serde_json::{Value, json};
use smol_macros::test;

use common::TEST_STRIP_LEN;

macro_rules! svc {
    ($store:expr) => {{
        common::router(
            &common::state($store, mock_runtime()),
            &[api::zones::routes],
        )
    }};
}

#[apply(test!)]
async fn crud() {
    let svc = svc!(make_store().await);

    // create
    let resp = svc
        .call(
            Request::post("/zones")
                .with_json(&json!({"name": "Full", "start_pixel": 0, "end_pixel": 100})),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = resp.json_body().unwrap();
    assert_eq!(body["name"], "Full");
    assert_eq!(body["transition_length"], 0);
    let id = body["id"].as_str().unwrap().to_string();

    // get
    let resp = svc.call(Request::get(&format!("/zones/{id}"))).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // update
    let resp = svc
        .call(Request::put(&format!("/zones/{id}")).with_json(&json!({
            "name": "Renamed",
            "start_pixel": 0,
            "end_pixel": 50,
            "transition_length": 8
        })))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json_body().unwrap();
    assert_eq!(body["name"], "Renamed");
    assert_eq!(body["transition_length"], 8);

    // list — includes the virtual "all" zone plus the one user zone
    let resp = svc.call(Request::get("/zones")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Vec<Value> = resp.json_body().unwrap();
    assert_eq!(body.len(), 2);

    // delete
    let resp = svc.call(Request::delete(&format!("/zones/{id}"))).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[apply(test!)]
async fn get_nonexistent_returns_404() {
    let svc = svc!(make_store().await);

    let resp = svc.call(Request::get("/zones/no-such-zone")).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[apply(test!)]
async fn all_zone_is_present_in_list() {
    let svc = svc!(make_store().await);

    let resp = svc.call(Request::get("/zones")).await;
    let body: Vec<Value> = resp.json_body().unwrap();
    let all = body.iter().find(|z| z["id"] == "all").unwrap();
    assert_eq!(all["name"], "Full Strip");
    assert_eq!(all["start_pixel"], 0);
    assert_eq!(all["end_pixel"], TEST_STRIP_LEN as u64 - 1);
}

#[apply(test!)]
async fn all_zone_get_returns_ok() {
    let svc = svc!(make_store().await);

    let resp = svc.call(Request::get("/zones/all")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json_body().unwrap();
    assert_eq!(body["id"], "all");
}

#[apply(test!)]
async fn all_zone_delete_returns_403() {
    let svc = svc!(make_store().await);

    let resp = svc.call(Request::delete("/zones/all")).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[apply(test!)]
async fn all_zone_update_returns_403() {
    let svc = svc!(make_store().await);

    let resp = svc
        .call(
            Request::put("/zones/all")
                .with_json(&json!({"name": "x", "start_pixel": 0, "end_pixel": 10})),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
