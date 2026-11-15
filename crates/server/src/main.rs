mod scene_runtime;
mod settings;

use std::sync::Arc;

use application::{SceneRuntime, StateEventBus};
use effects::load_builtins;
use engine::EffectQueue;
use lumehub_server::state_saver::StateSaver;
use settings::Settings;
use smol::net::TcpListener;
use store::Store;

use scene_runtime::RenderTaskRuntime;

fn main() -> std::io::Result<()> {
    configure_executor_threads();
    smol::block_on(run())
}

/// smol's global executor runs a single worker thread unless told otherwise.
/// Use one per core, like the tokio runtime this replaced, so a slow scene
/// reload can't stall HTTP or MQTT. `SMOL_THREADS` still overrides.
fn configure_executor_threads() {
    if std::env::var_os("SMOL_THREADS").is_none() {
        let threads = std::thread::available_parallelism().map_or(2, |n| n.get());
        // SAFETY: called first thing in main, before any other thread exists.
        unsafe { std::env::set_var("SMOL_THREADS", threads.to_string()) };
    }
}

async fn run() -> std::io::Result<()> {
    let app_settings = Settings::new().unwrap_or_else(|e| {
        eprintln!("error: failed to load configuration: {}", e);
        std::process::exit(1);
    });

    let led = match drivers::create(&app_settings.led_controller.driver) {
        Ok(controller) => controller,
        Err(e) => {
            eprintln!("Error creating LED controller: {}", e);
            Box::new(drivers::Console::new(
                app_settings.led_controller.driver.pixel_count,
            ))
        }
    };

    let strip_len = app_settings.led_controller.driver.pixel_count;

    let (effect_queue, rx_effect_processor) =
        EffectQueue::new(app_settings.led_controller.crossfade_ms);

    let state_file = app_settings.state_dir.join("state.json");
    let initial_state = persistence::load(&state_file).unwrap_or_else(|e| {
        eprintln!("warning: failed to load state: {}", e);
        persistence::PersistedState::default()
    });

    let db_path = app_settings.state_dir.join("lumehub.db");
    let store = Arc::new(Store::open(&db_path).await.unwrap_or_else(|e| {
        eprintln!("error: failed to open database: {}", e);
        std::process::exit(1);
    }));

    let builtins = load_builtins();
    for e in effects::validate_builtin_scripts() {
        eprintln!("warning: {}", e);
    }

    let event_bus = StateEventBus::new();
    let runtime: Arc<dyn SceneRuntime> = Arc::new(RenderTaskRuntime::new(
        effect_queue.clone(),
        event_bus.clone(),
        initial_state,
        Some(StateSaver::spawn(state_file)),
        Arc::clone(&store),
        builtins.clone(),
        strip_len,
    ));

    // Restore active scene from __active__ on startup
    runtime.reload_active();

    if app_settings.mqtt.enabled {
        let mqtt_builtins = load_builtins();
        mqtt::spawn(
            app_settings.mqtt.clone(),
            Arc::clone(&runtime),
            Arc::clone(&store),
            event_bus.clone(),
            mqtt_builtins,
        );
    }

    let crossfade_frames = app_settings.led_controller.crossfade_ms as usize / 16;
    engine::spawn(Box::new(led), rx_effect_processor, crossfade_frames);

    let ip_address = app_settings.server.ip_address;
    let port = app_settings.server.port;

    let mut router = httpd::Router::new();
    api::routes(
        &mut router,
        &api::ApiState {
            store: Arc::clone(&store),
            runtime: Arc::clone(&runtime),
            builtins: Arc::new(builtins),
            strip_len,
            events: event_bus.clone(),
        },
    );
    api_legacy::routes(&mut router, &runtime);
    #[cfg(feature = "google")]
    api_google::routes(
        &mut router,
        &api_google::GoogleState {
            runtime: Arc::clone(&runtime),
            dispatcher: api_google::commands::CommandDispatcher::new(),
        },
    );

    let listener = TcpListener::bind((ip_address.as_str(), port)).await?;
    println!("Server running at http://{}:{}", ip_address, port);

    let _mdns = app_settings.mdns.enable.then(|| {
        let mdns_config = discovery::Config {
            name: app_settings.mdns.name.clone(),
            port,
        };
        discovery::advertise(&mdns_config)
            .map_err(|e| eprintln!("warning: mDNS advertisement failed: {}", e))
            .ok()
    });

    httpd::serve(listener, Arc::new(router)).await
}
