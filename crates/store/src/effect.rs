use rusqlite::{Connection, OptionalExtension, Row, params};
use uuid::Uuid;

use domain::ParamDef;

use crate::StoreError;

#[derive(Debug, Clone)]
pub struct EffectRecord {
    pub id: String,
    pub name: String,
    pub script: String,
    pub params: Vec<ParamDef>,
}

struct EffectRow {
    id: String,
    name: String,
    script: String,
    params: String,
}

fn read_row(r: &Row) -> rusqlite::Result<EffectRow> {
    Ok(EffectRow {
        id: r.get(0)?,
        name: r.get(1)?,
        script: r.get(2)?,
        params: r.get(3)?,
    })
}

fn from_row(row: EffectRow) -> Result<EffectRecord, StoreError> {
    Ok(EffectRecord {
        params: serde_json::from_str(&row.params)?,
        id: row.id,
        name: row.name,
        script: row.script,
    })
}

pub fn get_all(conn: &Connection) -> Result<Vec<EffectRecord>, StoreError> {
    let mut stmt = conn.prepare("SELECT id, name, script, params FROM effects ORDER BY name")?;
    let rows = stmt.query_map([], read_row)?;
    rows.map(|r| from_row(r?)).collect()
}

pub fn get_one(conn: &Connection, id: &str) -> Result<EffectRecord, StoreError> {
    let row = conn
        .query_row(
            "SELECT id, name, script, params FROM effects WHERE id = ?",
            [id],
            read_row,
        )
        .optional()?;
    from_row(row.ok_or(StoreError::NotFound)?)
}

pub fn create(
    conn: &Connection,
    name: &str,
    script: &str,
    params: &[ParamDef],
) -> Result<EffectRecord, StoreError> {
    let id = Uuid::new_v4().to_string();
    let params_json = serde_json::to_string(params)?;
    conn.execute(
        "INSERT INTO effects (id, name, script, params) VALUES (?, ?, ?, ?)",
        params![id, name, script, params_json],
    )?;
    get_one(conn, &id)
}

pub fn update(
    conn: &Connection,
    id: &str,
    name: &str,
    script: &str,
    params: &[ParamDef],
) -> Result<EffectRecord, StoreError> {
    let params_json = serde_json::to_string(params)?;
    let rows_affected = conn.execute(
        "UPDATE effects SET name = ?, script = ?, params = ? WHERE id = ?",
        params![name, script, params_json, id],
    )?;
    if rows_affected == 0 {
        return Err(StoreError::NotFound);
    }
    get_one(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<(), StoreError> {
    if conn.execute("DELETE FROM effects WHERE id = ?", [id])? == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}
