use std::path::PathBuf;

use clap::Parser;
use drivers::DriverConfig;
use lumehub_server::config_layers::{apply_env, merge};
use mqtt::MqttConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LedControllerConfig {
    #[serde(flatten)]
    pub driver: DriverConfig,
    pub crossfade_ms: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ServerConfig {
    pub ip_address: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MdnsConfig {
    pub enable: bool,
    pub port: u16,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Settings {
    pub led_controller: LedControllerConfig,
    pub server: ServerConfig,
    pub mdns: MdnsConfig,
    #[serde(default)]
    pub mqtt: MqttConfig,
    #[serde(skip)]
    pub config_dir: PathBuf,
    #[serde(skip)]
    pub state_dir: PathBuf,
}

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    #[arg(short, long, env = "LUMEHUB_CONFIG")]
    config: Option<String>,

    #[arg(long, env = "LUMEHUB_CONFIG_DIR", default_value = "config")]
    config_dir: String,

    #[arg(long, env = "LUMEHUB_STATE_DIR", default_value = ".")]
    state_dir: String,
}

impl Settings {
    pub fn new() -> Result<Self, String> {
        let args = Args::parse();

        let mut table: toml::Table = include_str!("../../../config/default.toml")
            .parse()
            .map_err(|e| format!("built-in defaults: {e}"))?;

        let path = args.config.unwrap_or_else(|| "config.toml".to_string());
        match std::fs::read_to_string(&path) {
            Ok(text) => merge(
                &mut table,
                text.parse().map_err(|e| format!("{path}: {e}"))?,
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{path}: {e}")),
        }

        apply_env(&mut table, "LUMEHUB", std::env::vars());

        let mut settings: Self = table.try_into().map_err(|e| format!("{e}"))?;
        settings.config_dir = PathBuf::from(args.config_dir);
        settings.state_dir = PathBuf::from(args.state_dir);
        Ok(settings)
    }
}
