//! Layered TOML configuration: built-in defaults <- config file <- environment.
//!
//! Replaces the `config` crate. Environment variables use `PREFIX_SECTION__KEY`
//! (double underscore between nesting levels), e.g. `LUMEHUB_MQTT__PASSWORD`.

use toml::{Table, Value};

/// Deep-merge `over` into `base`; tables merge recursively, everything else replaces.
pub fn merge(base: &mut Table, over: Table) {
    for (key, value) in over {
        match (base.get_mut(&key), value) {
            (Some(Value::Table(b)), Value::Table(o)) => merge(b, o),
            (_, v) => {
                base.insert(key, v);
            }
        }
    }
}

/// Apply `PREFIX_A__B=value` variables as `a.b = value`. Only variables with
/// at least one `__` are considered, so CLI-level vars like `PREFIX_STATE_DIR`
/// are left alone. The value is parsed to the type of the key it replaces;
/// keys that don't exist yet are set as strings.
pub fn apply_env(
    table: &mut Table,
    prefix: &str,
    vars: impl IntoIterator<Item = (String, String)>,
) {
    let prefix = format!("{prefix}_");
    for (name, raw) in vars {
        let Some(path) = name.strip_prefix(&prefix) else {
            continue;
        };
        if !path.contains("__") {
            continue;
        }
        let keys: Vec<String> = path.split("__").map(str::to_lowercase).collect();
        set_path(table, &keys, &raw);
    }
}

fn set_path(table: &mut Table, keys: &[String], raw: &str) {
    let (last, parents) = keys.split_last().expect("split yields at least one key");
    let mut cur = table;
    for k in parents {
        let entry = cur
            .entry(k.clone())
            .or_insert_with(|| Value::Table(Table::new()));
        let Value::Table(next) = entry else {
            eprintln!(
                "warning: config env override for '{}' ignored: '{k}' is not a table",
                keys.join(".")
            );
            return;
        };
        cur = next;
    }
    let value = typed_like(cur.get(last), raw);
    cur.insert(last.clone(), value);
}

fn typed_like(existing: Option<&Value>, raw: &str) -> Value {
    let parsed = match existing {
        Some(Value::Boolean(_)) => raw.parse().ok().map(Value::Boolean),
        Some(Value::Integer(_)) => raw.parse().ok().map(Value::Integer),
        Some(Value::Float(_)) => raw.parse().ok().map(Value::Float),
        _ => None,
    };
    parsed.unwrap_or_else(|| Value::String(raw.to_string()))
}
