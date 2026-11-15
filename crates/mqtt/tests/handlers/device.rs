use crate::common::{self, payload};
use domain::Rgb;
use macro_rules_attribute::apply;
use mqtt::handlers::route;
use serde_json::json;
use smol_macros::test;

#[apply(test!)]
async fn device_set_on_off() {
    let runtime = common::MockRuntime::new();
    let store = common::in_memory_store().await;
    let ctx = common::make_ctx(runtime.clone(), store).await;

    route(
        &ctx,
        "lumehub/device/set",
        &payload(json!({"state": "OFF"})),
    )
    .await;
    assert!(!*runtime.on.lock().unwrap());

    route(&ctx, "lumehub/device/set", &payload(json!({"state": "ON"}))).await;
    assert!(*runtime.on.lock().unwrap());
}

#[apply(test!)]
async fn device_set_brightness() {
    let runtime = common::MockRuntime::new();
    let store = common::in_memory_store().await;
    let ctx = common::make_ctx(runtime.clone(), store).await;

    route(
        &ctx,
        "lumehub/device/set",
        &payload(json!({"brightness": 128})),
    )
    .await;
    assert_eq!(*runtime.brightness.lock().unwrap(), 128);
}

#[apply(test!)]
async fn device_set_color() {
    let runtime = common::MockRuntime::new();
    let store = common::in_memory_store().await;
    let ctx = common::make_ctx(runtime.clone(), store).await;

    route(
        &ctx,
        "lumehub/device/set",
        &payload(json!({"color": {"r": 255, "g": 100, "b": 0}})),
    )
    .await;
    let color = *runtime.color.lock().unwrap();
    assert_eq!(
        color,
        Rgb {
            r: 255,
            g: 100,
            b: 0
        }
    );
}

#[apply(test!)]
async fn device_set_ignores_invalid_json() {
    let runtime = common::MockRuntime::new();
    let store = common::in_memory_store().await;
    let ctx = common::make_ctx(runtime.clone(), store).await;

    route(&ctx, "lumehub/device/set", b"not json").await;
    assert!(*runtime.on.lock().unwrap());
}
