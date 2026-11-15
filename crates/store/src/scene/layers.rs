use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension, Row, params};
use uuid::Uuid;

use domain::{BlendMode, ParamValue};

use crate::StoreError;

#[derive(Debug, Clone)]
pub struct LayerRecord {
    pub id: String,
    pub scene_id: String,
    pub effect_id: String,
    pub zone_id: String,
    pub blend_mode: BlendMode,
    pub params: HashMap<String, ParamValue>,
    pub enabled: bool,
    pub position: u32,
}

struct LayerRow {
    id: String,
    scene_id: String,
    effect_id: String,
    zone_id: String,
    blend_mode: String,
    params: String,
    enabled: i64,
    position: i64,
}

pub struct NewLayer {
    pub effect_id: String,
    pub zone_id: String,
    pub blend_mode: BlendMode,
    pub params: HashMap<String, ParamValue>,
}

const SELECT: &str =
    "SELECT id, scene_id, effect_id, zone_id, blend_mode, params, enabled, position
     FROM scene_layers";

pub(super) const INSERT: &str = "INSERT INTO scene_layers
     (id, scene_id, effect_id, zone_id, blend_mode, params, enabled, position)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?)";

fn read_row(r: &Row) -> rusqlite::Result<LayerRow> {
    Ok(LayerRow {
        id: r.get(0)?,
        scene_id: r.get(1)?,
        effect_id: r.get(2)?,
        zone_id: r.get(3)?,
        blend_mode: r.get(4)?,
        params: r.get(5)?,
        enabled: r.get(6)?,
        position: r.get(7)?,
    })
}

fn layer_from_row(row: LayerRow) -> Result<LayerRecord, StoreError> {
    Ok(LayerRecord {
        blend_mode: BlendMode::from(row.blend_mode.as_str()),
        params: serde_json::from_str(&row.params)?,
        enabled: row.enabled != 0,
        position: row.position as u32,
        id: row.id,
        scene_id: row.scene_id,
        effect_id: row.effect_id,
        zone_id: row.zone_id,
    })
}

pub(super) fn blend_mode_to_str(mode: BlendMode) -> &'static str {
    match mode {
        BlendMode::Override => "override",
        BlendMode::Add => "add",
        BlendMode::Screen => "screen",
        BlendMode::Multiply => "multiply",
    }
}

pub fn get_layers(conn: &Connection, scene_id: &str) -> Result<Vec<LayerRecord>, StoreError> {
    let mut stmt = conn.prepare(&format!("{SELECT} WHERE scene_id = ? ORDER BY position"))?;
    let rows = stmt.query_map([scene_id], read_row)?;
    rows.map(|r| layer_from_row(r?)).collect()
}

pub fn get_layer(conn: &Connection, id: &str) -> Result<LayerRecord, StoreError> {
    let row = conn
        .query_row(&format!("{SELECT} WHERE id = ?"), [id], read_row)
        .optional()?;
    layer_from_row(row.ok_or(StoreError::NotFound)?)
}

pub fn get_layer_in_scene(
    conn: &Connection,
    id: &str,
    scene_id: &str,
) -> Result<LayerRecord, StoreError> {
    let row = conn
        .query_row(
            &format!("{SELECT} WHERE id = ? AND scene_id = ?"),
            [id, scene_id],
            read_row,
        )
        .optional()?;
    layer_from_row(row.ok_or(StoreError::NotFound)?)
}

pub fn add_layer(
    conn: &Connection,
    scene_id: &str,
    effect_id: &str,
    zone_id: &str,
    blend_mode: BlendMode,
    params: &HashMap<String, ParamValue>,
) -> Result<LayerRecord, StoreError> {
    let id = Uuid::new_v4().to_string();
    let params_json = serde_json::to_string(params)?;

    let position: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM scene_layers WHERE scene_id = ?",
        [scene_id],
        |r| r.get(0),
    )?;

    conn.execute(
        INSERT,
        params![
            id,
            scene_id,
            effect_id,
            zone_id,
            blend_mode_to_str(blend_mode),
            params_json,
            1i64,
            position
        ],
    )?;

    get_layer(conn, &id)
}

pub fn update_layer(
    conn: &Connection,
    id: &str,
    scene_id: &str,
    zone_id: &str,
    enabled: bool,
    blend_mode: BlendMode,
    params: &HashMap<String, ParamValue>,
) -> Result<LayerRecord, StoreError> {
    let params_json = serde_json::to_string(params)?;
    let rows_affected = conn.execute(
        "UPDATE scene_layers SET zone_id = ?, enabled = ?, blend_mode = ?, params = ?
         WHERE id = ? AND scene_id = ?",
        params![
            zone_id,
            enabled as i64,
            blend_mode_to_str(blend_mode),
            params_json,
            id,
            scene_id
        ],
    )?;
    if rows_affected == 0 {
        return Err(StoreError::NotFound);
    }
    get_layer(conn, id)
}

pub fn remove_layer(conn: &Connection, id: &str, scene_id: &str) -> Result<(), StoreError> {
    let rows_affected = conn.execute(
        "DELETE FROM scene_layers WHERE id = ? AND scene_id = ?",
        [id, scene_id],
    )?;
    if rows_affected == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

pub fn reorder_layers(
    conn: &Connection,
    scene_id: &str,
    ordered_ids: &[String],
) -> Result<(), StoreError> {
    if ordered_ids.is_empty() {
        return Ok(());
    }

    let existing: HashSet<String> = get_layers(conn, scene_id)?
        .into_iter()
        .map(|l| l.id)
        .collect();
    if !ordered_ids.iter().all(|id| existing.contains(id)) {
        return Err(StoreError::NotFound);
    }

    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt =
            tx.prepare("UPDATE scene_layers SET position = ? WHERE id = ? AND scene_id = ?")?;
        for (position, id) in ordered_ids.iter().enumerate() {
            stmt.execute(params![position as i64, id, scene_id])?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn clear_layers(conn: &Connection, scene_id: &str) -> Result<(), StoreError> {
    conn.execute("DELETE FROM scene_layers WHERE scene_id = ?", [scene_id])?;
    Ok(())
}

/// Copy every layer of `from_scene_id` into `to_scene_id`, preserving order,
/// enabled flag and params. Callers wrap this in a transaction when it must be
/// atomic with other writes.
pub(super) fn copy_layers(
    conn: &Connection,
    from_scene_id: &str,
    to_scene_id: &str,
) -> Result<(), StoreError> {
    let layers = get_layers(conn, from_scene_id)?;
    let mut stmt = conn.prepare(INSERT)?;
    for l in &layers {
        stmt.execute(params![
            Uuid::new_v4().to_string(),
            to_scene_id,
            l.effect_id,
            l.zone_id,
            blend_mode_to_str(l.blend_mode),
            serde_json::to_string(&l.params)?,
            l.enabled as i64,
            l.position as i64
        ])?;
    }
    Ok(())
}
