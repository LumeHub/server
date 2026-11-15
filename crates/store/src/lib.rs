mod effect;
mod scene;
mod zone;

pub use effect::EffectRecord;
pub use scene::{ACTIVE_SCENE_ID, LayerRecord, NewLayer, SceneRecord};
pub use zone::ZoneRecord;

use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use thiserror::Error;

/// Schema version written to `PRAGMA user_version` once migrations ran.
/// Databases created by the old sqlx store report 0 here; `0001` is fully
/// idempotent (`IF NOT EXISTS` / `INSERT OR IGNORE`) so they upgrade in place.
const SCHEMA_VERSION: i64 = 1;
const MIGRATION_0001: &str = include_str!("../migrations/0001_initial.sql");

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("not found")]
    NotFound,
}

/// SQLite-backed store. Queries are sub-millisecond on the tiny LumeHub
/// database, so they run inline under one shared connection rather than on a
/// thread pool; the async signatures are kept so callers stay runtime-agnostic.
#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
}

impl Store {
    pub async fn open(path: &Path) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        // sqlx defaulted to WAL; keep it so readers never block the writer.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::init(conn)
    }

    pub async fn open_in_memory() -> Result<Self, StoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, StoreError> {
        // sqlx enabled foreign keys by default; scene deletion relies on the
        // `ON DELETE CASCADE` from scene_layers -> scenes.
        conn.pragma_update(None, "foreign_keys", true)?;
        migrate(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn with<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        f(&conn)
    }
}

fn migrate(conn: &Connection) -> Result<(), StoreError> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version < 1 {
        conn.execute_batch(MIGRATION_0001)?;
    }
    if version < SCHEMA_VERSION {
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }
    Ok(())
}

// Delegate CRUD methods so callers only need the Store handle.
impl Store {
    pub async fn get_zones(&self) -> Result<Vec<ZoneRecord>, StoreError> {
        self.with(zone::get_all)
    }
    pub async fn get_zone(&self, id: &str) -> Result<ZoneRecord, StoreError> {
        self.with(|c| zone::get_one(c, id))
    }
    pub async fn create_zone(
        &self,
        name: &str,
        start_pixel: u32,
        end_pixel: u32,
        transition_length: u32,
    ) -> Result<ZoneRecord, StoreError> {
        self.with(|c| zone::create(c, name, start_pixel, end_pixel, transition_length))
    }
    pub async fn update_zone(
        &self,
        id: &str,
        name: &str,
        start_pixel: u32,
        end_pixel: u32,
        transition_length: u32,
    ) -> Result<ZoneRecord, StoreError> {
        self.with(|c| zone::update(c, id, name, start_pixel, end_pixel, transition_length))
    }
    pub async fn delete_zone(&self, id: &str) -> Result<(), StoreError> {
        self.with(|c| zone::delete(c, id))
    }

    pub async fn get_effects(&self) -> Result<Vec<EffectRecord>, StoreError> {
        self.with(effect::get_all)
    }
    pub async fn get_effect(&self, id: &str) -> Result<EffectRecord, StoreError> {
        self.with(|c| effect::get_one(c, id))
    }
    pub async fn create_effect(
        &self,
        name: &str,
        script: &str,
        params: &[domain::ParamDef],
    ) -> Result<EffectRecord, StoreError> {
        self.with(|c| effect::create(c, name, script, params))
    }
    pub async fn update_effect(
        &self,
        id: &str,
        name: &str,
        script: &str,
        params: &[domain::ParamDef],
    ) -> Result<EffectRecord, StoreError> {
        self.with(|c| effect::update(c, id, name, script, params))
    }
    pub async fn delete_effect(&self, id: &str) -> Result<(), StoreError> {
        self.with(|c| effect::delete(c, id))
    }

    pub async fn get_scenes(&self) -> Result<Vec<SceneRecord>, StoreError> {
        self.with(scene::get_all)
    }
    pub async fn get_scene(&self, id: &str) -> Result<SceneRecord, StoreError> {
        self.with(|c| scene::get_one(c, id))
    }
    pub async fn create_scene(&self, name: &str) -> Result<SceneRecord, StoreError> {
        self.with(|c| scene::create(c, name))
    }
    pub async fn update_scene(&self, id: &str, name: &str) -> Result<SceneRecord, StoreError> {
        self.with(|c| scene::update(c, id, name))
    }
    pub async fn delete_scene(&self, id: &str) -> Result<(), StoreError> {
        self.with(|c| scene::delete(c, id))
    }

    pub async fn get_layers(&self, scene_id: &str) -> Result<Vec<LayerRecord>, StoreError> {
        self.with(|c| scene::get_layers(c, scene_id))
    }
    pub async fn add_layer(
        &self,
        scene_id: &str,
        effect_id: &str,
        zone_id: &str,
        blend_mode: domain::BlendMode,
        params: &std::collections::HashMap<String, domain::ParamValue>,
    ) -> Result<LayerRecord, StoreError> {
        self.with(|c| scene::add_layer(c, scene_id, effect_id, zone_id, blend_mode, params))
    }
    pub async fn update_layer(
        &self,
        id: &str,
        scene_id: &str,
        zone_id: &str,
        enabled: bool,
        blend_mode: domain::BlendMode,
        params: &std::collections::HashMap<String, domain::ParamValue>,
    ) -> Result<LayerRecord, StoreError> {
        self.with(|c| scene::update_layer(c, id, scene_id, zone_id, enabled, blend_mode, params))
    }
    pub async fn remove_layer(&self, id: &str, scene_id: &str) -> Result<(), StoreError> {
        self.with(|c| scene::remove_layer(c, id, scene_id))
    }

    pub async fn get_active_layer(&self, id: &str) -> Result<LayerRecord, StoreError> {
        self.with(|c| scene::get_layer_in_scene(c, id, ACTIVE_SCENE_ID))
    }
    pub async fn update_active_layer(
        &self,
        id: &str,
        zone_id: &str,
        enabled: bool,
        blend_mode: domain::BlendMode,
        params: &std::collections::HashMap<String, domain::ParamValue>,
    ) -> Result<LayerRecord, StoreError> {
        self.with(|c| {
            scene::update_layer(c, id, ACTIVE_SCENE_ID, zone_id, enabled, blend_mode, params)
        })
    }
    pub async fn remove_active_layer(&self, id: &str) -> Result<(), StoreError> {
        self.with(|c| scene::remove_layer(c, id, ACTIVE_SCENE_ID))
    }
    pub async fn reorder_layers(
        &self,
        scene_id: &str,
        ordered_ids: &[String],
    ) -> Result<(), StoreError> {
        self.with(|c| scene::reorder_layers(c, scene_id, ordered_ids))
    }

    pub async fn get_layer_by_id(&self, id: &str) -> Result<LayerRecord, StoreError> {
        self.with(|c| scene::get_layer(c, id))
    }

    pub async fn get_active_layers(&self) -> Result<Vec<LayerRecord>, StoreError> {
        self.with(|c| scene::get_layers(c, ACTIVE_SCENE_ID))
    }
    pub async fn replace_active_layers(&self, layers: &[NewLayer]) -> Result<(), StoreError> {
        self.with(|c| scene::replace_active_layers(c, layers))
    }
    pub async fn add_active_layer(
        &self,
        effect_id: &str,
        zone_id: &str,
        blend_mode: domain::BlendMode,
        params: &std::collections::HashMap<String, domain::ParamValue>,
    ) -> Result<LayerRecord, StoreError> {
        self.with(|c| scene::add_layer(c, ACTIVE_SCENE_ID, effect_id, zone_id, blend_mode, params))
    }
    pub async fn reorder_active_layers(&self, ordered_ids: &[String]) -> Result<(), StoreError> {
        self.with(|c| scene::reorder_layers(c, ACTIVE_SCENE_ID, ordered_ids))
    }
    pub async fn clear_active_scene(&self) -> Result<(), StoreError> {
        self.with(|c| scene::clear_layers(c, ACTIVE_SCENE_ID))
    }
    pub async fn load_scene_into_active(&self, scene_id: &str) -> Result<(), StoreError> {
        self.with(|c| scene::load_into_active(c, scene_id))
    }
    pub async fn save_active_as_scene(&self, name: &str) -> Result<SceneRecord, StoreError> {
        self.with(|c| scene::save_active_as(c, name))
    }
    pub async fn overwrite_scene_from_active(&self, id: &str) -> Result<(), StoreError> {
        self.with(|c| scene::overwrite_from_active(c, id))
    }
}
