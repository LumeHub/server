use rusqlite::{Connection, OptionalExtension, Row, params};
use uuid::Uuid;

use crate::StoreError;

#[derive(Debug, Clone)]
pub struct SceneRecord {
    pub id: String,
    pub name: String,
}

fn from_row(r: &Row) -> rusqlite::Result<SceneRecord> {
    Ok(SceneRecord {
        id: r.get(0)?,
        // The reserved active-scene row has a NULL name.
        name: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
    })
}

pub fn get_all(conn: &Connection) -> Result<Vec<SceneRecord>, StoreError> {
    let mut stmt = conn.prepare("SELECT id, name FROM scenes WHERE id != ? ORDER BY name")?;
    let rows = stmt.query_map([super::ACTIVE_SCENE_ID], from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get_one(conn: &Connection, id: &str) -> Result<SceneRecord, StoreError> {
    conn.query_row("SELECT id, name FROM scenes WHERE id = ?", [id], from_row)
        .optional()?
        .ok_or(StoreError::NotFound)
}

pub fn create(conn: &Connection, name: &str) -> Result<SceneRecord, StoreError> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO scenes (id, name) VALUES (?, ?)",
        params![id, name],
    )?;
    get_one(conn, &id)
}

pub fn update(conn: &Connection, id: &str, name: &str) -> Result<SceneRecord, StoreError> {
    let rows_affected = conn.execute(
        "UPDATE scenes SET name = ? WHERE id = ? AND id != ?",
        params![name, id, super::ACTIVE_SCENE_ID],
    )?;
    if rows_affected == 0 {
        return Err(StoreError::NotFound);
    }
    get_one(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<(), StoreError> {
    let rows_affected = conn.execute(
        "DELETE FROM scenes WHERE id = ? AND id != ?",
        params![id, super::ACTIVE_SCENE_ID],
    )?;
    if rows_affected == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}
