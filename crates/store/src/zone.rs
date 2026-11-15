use rusqlite::{Connection, OptionalExtension, Row, params};
use uuid::Uuid;

use crate::StoreError;

#[derive(Debug, Clone)]
pub struct ZoneRecord {
    pub id: String,
    pub name: String,
    pub start_pixel: u32,
    pub end_pixel: u32,
    pub transition_length: u32,
}

const COLUMNS: &str = "id, name, start_pixel, end_pixel, transition_length";

fn from_row(r: &Row) -> rusqlite::Result<ZoneRecord> {
    Ok(ZoneRecord {
        id: r.get(0)?,
        name: r.get(1)?,
        start_pixel: r.get::<_, i64>(2)? as u32,
        end_pixel: r.get::<_, i64>(3)? as u32,
        transition_length: r.get::<_, i64>(4)? as u32,
    })
}

pub fn get_all(conn: &Connection) -> Result<Vec<ZoneRecord>, StoreError> {
    let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM zones ORDER BY name"))?;
    let rows = stmt.query_map([], from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get_one(conn: &Connection, id: &str) -> Result<ZoneRecord, StoreError> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM zones WHERE id = ?"),
        [id],
        from_row,
    )
    .optional()?
    .ok_or(StoreError::NotFound)
}

pub fn create(
    conn: &Connection,
    name: &str,
    start_pixel: u32,
    end_pixel: u32,
    transition_length: u32,
) -> Result<ZoneRecord, StoreError> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO zones (id, name, start_pixel, end_pixel, transition_length)
         VALUES (?, ?, ?, ?, ?)",
        params![id, name, start_pixel, end_pixel, transition_length],
    )?;
    get_one(conn, &id)
}

pub fn update(
    conn: &Connection,
    id: &str,
    name: &str,
    start_pixel: u32,
    end_pixel: u32,
    transition_length: u32,
) -> Result<ZoneRecord, StoreError> {
    let rows_affected = conn.execute(
        "UPDATE zones SET name = ?, start_pixel = ?, end_pixel = ?, transition_length = ?
         WHERE id = ?",
        params![name, start_pixel, end_pixel, transition_length, id],
    )?;
    if rows_affected == 0 {
        return Err(StoreError::NotFound);
    }
    get_one(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<(), StoreError> {
    if conn.execute("DELETE FROM zones WHERE id = ?", [id])? == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}
