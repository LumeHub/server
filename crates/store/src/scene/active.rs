use rusqlite::{Connection, params};
use uuid::Uuid;

use crate::StoreError;

use super::layers::{INSERT, NewLayer, blend_mode_to_str, clear_layers, copy_layers};
use super::scenes::{SceneRecord, create, get_one};

pub fn replace_active_layers(conn: &Connection, layers: &[NewLayer]) -> Result<(), StoreError> {
    let tx = conn.unchecked_transaction()?;
    clear_layers(&tx, super::ACTIVE_SCENE_ID)?;
    {
        let mut stmt = tx.prepare(INSERT)?;
        for (pos, l) in layers.iter().enumerate() {
            stmt.execute(params![
                Uuid::new_v4().to_string(),
                super::ACTIVE_SCENE_ID,
                l.effect_id,
                l.zone_id,
                blend_mode_to_str(l.blend_mode),
                serde_json::to_string(&l.params)?,
                1i64,
                pos as i64
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Replace the active layers with a copy of `scene_id`'s layers, atomically:
/// a failure halfway leaves the previous active layers in place.
pub fn load_into_active(conn: &Connection, scene_id: &str) -> Result<(), StoreError> {
    get_one(conn, scene_id)?;
    let tx = conn.unchecked_transaction()?;
    clear_layers(&tx, super::ACTIVE_SCENE_ID)?;
    copy_layers(&tx, scene_id, super::ACTIVE_SCENE_ID)?;
    tx.commit()?;
    Ok(())
}

pub fn save_active_as(conn: &Connection, name: &str) -> Result<SceneRecord, StoreError> {
    let tx = conn.unchecked_transaction()?;
    let scene = create(&tx, name)?;
    copy_layers(&tx, super::ACTIVE_SCENE_ID, &scene.id)?;
    tx.commit()?;
    Ok(scene)
}

pub fn overwrite_from_active(conn: &Connection, id: &str) -> Result<(), StoreError> {
    if id == super::ACTIVE_SCENE_ID {
        return Err(StoreError::NotFound);
    }
    let tx = conn.unchecked_transaction()?;
    clear_layers(&tx, id)?;
    copy_layers(&tx, super::ACTIVE_SCENE_ID, id)?;
    tx.commit()?;
    Ok(())
}
