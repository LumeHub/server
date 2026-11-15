use std::sync::{Arc, Mutex};
use std::time::Duration;

use application::{SceneRuntime, StateEventBus};
use effects::BuiltinEffect;
use store::Store;

use crate::config::MqttConfig;
use crate::ctx::Ctx;
use crate::discovery::publish_ha_discovery;
use crate::handlers::dispatch;
use crate::publish::{publish_all_initial, publish_device_state};
use crate::topics::Topics;
use crate::transport::{self, Connection, ConnectionError, MqttClient, Options};

const KEEP_ALIVE: Duration = Duration::from_secs(60);
const RECONNECT_DELAY: Duration = Duration::from_secs(5);
/// Retained effect lists with embedded scripts can get large.
const MAX_PACKET_SIZE: usize = 1024 * 1024;

pub async fn run(
    config: MqttConfig,
    runtime: Arc<dyn SceneRuntime>,
    store: Arc<Store>,
    event_bus: StateEventBus,
    builtins: Vec<BuiltinEffect>,
) {
    let builtins = Arc::new(builtins);
    let opts = options(&config);

    loop {
        match transport::connect(&opts).await {
            Ok((client, conn)) => {
                println!("[mqtt] connected to {}:{}", config.host, config.port);
                let ctx = Ctx {
                    client,
                    topics: Topics::new(&config.topic_prefix),
                    config: config.clone(),
                    runtime: Arc::clone(&runtime),
                    store: Arc::clone(&store),
                    builtins: Arc::clone(&builtins),
                    active_effect: Arc::new(Mutex::new(None)),
                };
                let err = serve(ctx, conn, &event_bus).await;
                eprintln!("[mqtt] disconnected: {err}");
            }
            Err(e) => eprintln!("[mqtt] connection failed: {e}"),
        }
        smol::Timer::after(RECONNECT_DELAY).await;
    }
}

fn options(config: &MqttConfig) -> Options {
    let non_empty = |s: &Option<String>| s.clone().filter(|s| !s.is_empty());
    let (username, password) = match (non_empty(&config.username), non_empty(&config.password)) {
        (Some(u), Some(p)) => (Some(u), Some(p)),
        _ => (None, None),
    };
    Options {
        host: config.host.clone(),
        port: config.port,
        client_id: format!("lumehub-{}", config.device_id),
        keep_alive: KEEP_ALIVE,
        username,
        password,
        max_packet_size: MAX_PACKET_SIZE,
    }
}

/// Handle one live connection until it fails. Background tasks are owned
/// here, so they stop as soon as the connection does.
async fn serve(ctx: Ctx, mut conn: Connection, event_bus: &StateEventBus) -> ConnectionError {
    subscribe_all(&ctx.client, &ctx.topics).await;

    // Push retained state in the background so commands are served meanwhile.
    let _initial = smol::spawn(publish_initial(
        ctx.client.clone(),
        ctx.config.clone(),
        Arc::clone(&ctx.runtime),
        Arc::clone(&ctx.store),
        Arc::clone(&ctx.builtins),
    ));
    let _state = smol::spawn(forward_device_state(
        ctx.client.clone(),
        Topics::new(&ctx.topics.prefix),
        Arc::clone(&ctx.active_effect),
        event_bus.clone(),
    ));

    loop {
        match conn.next_publish().await {
            Ok(msg) => dispatch(&ctx, msg).await,
            Err(e) => return e,
        }
    }
}

async fn publish_initial(
    client: MqttClient,
    config: MqttConfig,
    runtime: Arc<dyn SceneRuntime>,
    store: Arc<Store>,
    builtins: Arc<Vec<BuiltinEffect>>,
) {
    let topics = Topics::new(&config.topic_prefix);
    println!("[mqtt] publishing retained state...");
    publish_all_initial(&client, &topics, &runtime, &store, &builtins).await;
    if config.ha_discovery {
        println!("[mqtt] publishing HA discovery...");
        publish_ha_discovery(&client, &config, &store, &builtins).await;
        println!("[mqtt] HA discovery published");
    }
}

async fn forward_device_state(
    client: MqttClient,
    topics: Topics,
    active_effect: Arc<Mutex<Option<String>>>,
    event_bus: StateEventBus,
) {
    let mut snapshots = event_bus.subscribe();
    while let Some(snapshot) = snapshots.recv().await {
        let effect = active_effect.lock().unwrap().clone();
        publish_device_state(&client, &topics, &snapshot, effect.as_deref()).await;
    }
}

async fn subscribe_all(client: &MqttClient, topics: &Topics) {
    let subs = [
        topics.device_set(),
        topics.zones_create(),
        topics.zones_update_wildcard(),
        topics.zones_delete_wildcard(),
        topics.scenes_create(),
        topics.scenes_update_wildcard(),
        topics.scenes_delete_wildcard(),
        topics.scenes_load_wildcard(),
        topics.scenes_save_wildcard(),
        topics.active_layers_set(),
        topics.active_layers_clear(),
        topics.active_layers_add(),
        topics.active_layers_reorder(),
        topics.active_layers_update_wildcard(),
        topics.active_layers_delete_wildcard(),
        topics.effects_create(),
        topics.effects_update_wildcard(),
        topics.effects_delete_wildcard(),
        topics.active_scene_set(),
    ];
    let refs: Vec<&str> = subs.iter().map(String::as_str).collect();
    if let Err(e) = client.subscribe(&refs).await {
        eprintln!("[mqtt] subscribe error: {e}");
    }
}
