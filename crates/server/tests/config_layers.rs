use lumehub_server::config_layers::{apply_env, merge};
use toml::{Table, Value};

fn table(s: &str) -> Table {
    s.parse().unwrap()
}

fn vars(v: &[(&str, &str)]) -> Vec<(String, String)> {
    v.iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn merge_overrides_nested_keys_and_keeps_the_rest() {
    let mut base = table("[mqtt]\nenabled = false\nhost = \"localhost\"\n[server]\nport = 5000");
    merge(&mut base, table("[mqtt]\nenabled = true"));
    assert_eq!(base["mqtt"]["enabled"], Value::Boolean(true));
    assert_eq!(base["mqtt"]["host"], Value::String("localhost".into()));
    assert_eq!(base["server"]["port"], Value::Integer(5000));
}

#[test]
fn env_sets_nested_key_with_double_underscore() {
    let mut t = table("[mqtt]\nhost = \"localhost\"");
    apply_env(
        &mut t,
        "LUMEHUB",
        vars(&[("LUMEHUB_MQTT__PASSWORD", "1234")]),
    );
    // New keys stay strings even when they look numeric (passwords!).
    assert_eq!(t["mqtt"]["password"], Value::String("1234".into()));
}

#[test]
fn env_value_takes_type_of_existing_key() {
    let mut t = table("[mqtt]\nenabled = false\nport = 1883");
    apply_env(
        &mut t,
        "LUMEHUB",
        vars(&[
            ("LUMEHUB_MQTT__ENABLED", "true"),
            ("LUMEHUB_MQTT__PORT", "1884"),
        ]),
    );
    assert_eq!(t["mqtt"]["enabled"], Value::Boolean(true));
    assert_eq!(t["mqtt"]["port"], Value::Integer(1884));
}

#[test]
fn env_ignores_unrelated_and_cli_vars() {
    let mut t = table("[mqtt]\nhost = \"localhost\"");
    let before = t.clone();
    apply_env(
        &mut t,
        "LUMEHUB",
        vars(&[
            ("LUMEHUB_STATE_DIR", "/var/lib"),
            ("PATH", "/bin"),
            ("OTHER_X__Y", "1"),
        ]),
    );
    assert_eq!(t, before);
}
