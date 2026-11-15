use domain::ParamValue;
use store::Store;

pub async fn in_memory_store() -> Store {
    Store::open_in_memory().await.unwrap()
}

pub fn make_params() -> std::collections::HashMap<String, ParamValue> {
    [("speed".to_string(), ParamValue::Number(2.0))].into()
}
