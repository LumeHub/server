use macro_rules_attribute::apply;
use smol_macros::test;
use store::Store;

fn temp_db() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lumehub.db");
    (dir, path)
}

#[apply(test!)]
async fn file_store_persists_across_reopen() {
    let (_dir, path) = temp_db();
    {
        let store = Store::open(&path).await.unwrap();
        store.create_zone("desk", 0, 10, 4).await.unwrap();
    }
    let store = Store::open(&path).await.unwrap();
    let zones = store.get_zones().await.unwrap();
    assert_eq!(zones.len(), 1);
    assert_eq!(zones[0].name, "desk");
}

/// A prod database written by the old sqlx-based store: same schema plus
/// sqlx's own `_sqlx_migrations` bookkeeping table. It must open unchanged.
#[apply(test!)]
async fn opens_database_created_by_sqlx() {
    let (_dir, path) = temp_db();
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(include_str!("../migrations/0001_initial.sql"))
            .unwrap();
        conn.execute_batch(
            "CREATE TABLE _sqlx_migrations (
                version BIGINT PRIMARY KEY, description TEXT NOT NULL,
                installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                success BOOLEAN NOT NULL, checksum BLOB NOT NULL,
                execution_time BIGINT NOT NULL);
             INSERT INTO _sqlx_migrations VALUES (1, 'initial', CURRENT_TIMESTAMP, 1, x'00', 0);
             INSERT INTO scenes (id, name) VALUES ('s1', 'Sunset');
             INSERT INTO scene_layers (id, scene_id, effect_id, zone_id, blend_mode, params, enabled, position)
                 VALUES ('l1', 's1', 'fill', 'all', 'add', '{}', 1, 0);",
        )
        .unwrap();
    }

    let store = Store::open(&path).await.unwrap();
    let scenes = store.get_scenes().await.unwrap();
    assert_eq!(scenes.len(), 1);
    assert_eq!(scenes[0].name, "Sunset");
    let layers = store.get_layers("s1").await.unwrap();
    assert_eq!(layers.len(), 1);
    assert!(matches!(layers[0].blend_mode, domain::BlendMode::Add));
}

#[apply(test!)]
async fn in_memory_store_has_active_scene_row() {
    let store = Store::open_in_memory().await.unwrap();
    // The reserved active scene exists but is never listed as a user scene.
    assert!(store.get_scenes().await.unwrap().is_empty());
    assert!(store.get_active_layers().await.unwrap().is_empty());
}
