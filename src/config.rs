use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};
use valence::prelude::ConnectionMode;

/// The full server configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Resource)]
#[serde(default)]
pub(crate) struct ServerConfig {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) motd: Vec<String>,
    pub(crate) max_players: u32,
    pub(crate) online_mode: bool,
    pub(crate) whitelist: bool,
    pub(crate) chunk_render_distance: u32,
    pub(crate) default_gamemode: String,
    pub(crate) network_compression_threshold: i32,
    pub(crate) compression_enabled: bool,
    pub(crate) world: String,
    pub(crate) spawn: SpawnConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SpawnConfig {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) z: i32,
    pub(crate) terrain_radius: i32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".into(),
            port: 25565,
            motd: vec![
                "Welcome to Mili-rust!".into(),
                "A Minecraft Server in Rust".into(),
            ],
            max_players: 20,
            online_mode: false,
            whitelist: false,
            chunk_render_distance: 5,
            default_gamemode: "survival".into(),
            network_compression_threshold: -1,
            compression_enabled: false,
            world: "world".into(),
            spawn: SpawnConfig::default(),
        }
    }
}

impl Default for SpawnConfig {
    fn default() -> Self {
        Self {
            x: 0,
            y: 64,
            z: 0,
            terrain_radius: 50,
        }
    }
}

impl ServerConfig {
    pub(crate) fn load_or_create<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref();
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(content) => match toml::from_str::<ServerConfig>(&content) {
                    Ok(config) => {
                        println!("已加载配置文件: {}", path.display());
                        return config;
                    }
                    Err(e) => {
                        eprintln!("配置文件解析失败: {e}, 使用默认配置");
                    }
                },
                Err(e) => {
                    eprintln!("读取配置文件失败: {e}, 使用默认配置");
                }
            }
        } else {
            println!("未找到配置文件，正在生成默认配置: {}", path.display());
        }

        let config = ServerConfig::default();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(path, toml::to_string_pretty(&config).unwrap()) {
            Ok(()) => println!("已生成默认配置文件: {}", path.display()),
            Err(e) => eprintln!("写入配置文件失败: {e}"),
        }
        config
    }

    pub(crate) fn connection_mode(&self) -> ConnectionMode {
        if self.online_mode {
            ConnectionMode::Online {
                prevent_proxy_connections: false,
            }
        } else {
            ConnectionMode::Offline
        }
    }
}