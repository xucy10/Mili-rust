use std::path::Path;

use serde::{Deserialize, Serialize};
use valence::prelude::ConnectionMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub server: ServerSection,
    pub world: WorldSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerSection {
    pub port: u16,
    pub max_players: usize,
    pub online_mode: bool,
    pub motd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WorldSection {
    pub spawn_x: i32,
    pub spawn_y: i32,
    pub spawn_z: i32,
    pub terrain_radius: i32,
    pub chunk_radius: i32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server: ServerSection::default(),
            world: WorldSection::default(),
        }
    }
}

impl Default for ServerSection {
    fn default() -> Self {
        Self {
            port: 25565,
            max_players: 20,
            online_mode: true,
            motd: "Mili-rust Server".into(),
        }
    }
}

impl Default for WorldSection {
    fn default() -> Self {
        Self {
            spawn_x: 0,
            spawn_y: 64,
            spawn_z: 0,
            terrain_radius: 50,
            chunk_radius: 5,
        }
    }
}

impl ServerConfig {
    pub fn load_or_create<P: AsRef<Path>>(path: P) -> Self {
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

    pub fn connection_mode(&self) -> ConnectionMode {
        if self.server.online_mode {
            ConnectionMode::Online {
                prevent_proxy_connections: false,
            }
        } else {
            ConnectionMode::Offline
        }
    }
}
