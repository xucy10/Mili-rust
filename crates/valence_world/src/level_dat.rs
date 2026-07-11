use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use valence_nbt::binary::{from_binary, to_binary};
use valence_nbt::{Compound, Value};

/// Error type for level.dat operations.
#[derive(Debug, thiserror::Error)]
pub enum LevelDatError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("NBT error: {0}")]
    Nbt(#[from] valence_nbt::Error),
    #[error("missing field: {0}")]
    MissingField(String),
    #[error("invalid type for field: {0}")]
    InvalidType(String),
}

type Result<T> = std::result::Result<T, LevelDatError>;

/// Represents the complete level.dat file.
#[derive(Debug, Clone)]
pub struct LevelDat {
    pub data_version: i32,
    pub level_data: LevelData,
}

/// World-level data stored in level.dat.
#[derive(Debug, Clone)]
pub struct LevelData {
    pub allow_commands: bool,
    pub border_center_x: f64,
    pub border_center_z: f64,
    pub border_damage_per_block: f64,
    pub border_danger_zone_time: f64,
    pub border_size: f64,
    pub border_safe_zone: f64,
    pub border_teleport_boundary: f64,
    pub border_warning_blocks: f64,
    pub border_warning_time: f64,
    pub clear_weather_time: i32,
    pub difficulty: i8,
    pub difficulty_locked: bool,
    pub border_lerp_target: f64,
    pub border_lerp_time: i64,
    pub game_type: i32,
    pub generator_name: String,
    pub generator_options: Compound,
    pub generator_version: i32,
    pub hardcoded_spawn_allowed: bool,
    pub last_played: i64,
    pub level_name: String,
    pub linear_redstone: bool,
    pub max_players: i32,
    pub mob_spawn_range: i32,
    pub player_nether_scale: f32,
    pub raining: bool,
    pub rain_time: i32,
    pub spawn_angle: f32,
    pub spawn_x: i32,
    pub spawn_y: i32,
    pub spawn_z: i32,
    pub thundering: bool,
    pub thunder_time: i32,
    pub version: Compound,
    pub was_modded: bool,
}

impl Default for LevelData {
    fn default() -> Self {
        Self {
            allow_commands: true,
            border_center_x: 0.0,
            border_center_z: 0.0,
            border_damage_per_block: 0.2,
            border_danger_zone_time: 5.0,
            border_size: 60000000.0,
            border_safe_zone: 5.0,
            border_teleport_boundary: 29999984.0,
            border_warning_blocks: 5,
            border_warning_time: 5.0,
            clear_weather_time: 0,
            difficulty: 2, // Normal
            difficulty_locked: false,
            border_lerp_target: 60000000.0,
            border_lerp_time: 0,
            game_type: 0, // Survival
            generator_name: "default".to_owned(),
            generator_options: Compound::new(),
            generator_version: 1,
            hardcoded_spawn_allowed: false,
            last_played: 0,
            level_name: "world".to_owned(),
            linear_redstone: false,
            max_players: 20,
            mob_spawn_range: 8,
            player_nether_scale: 8.0,
            raining: false,
            rain_time: 0,
            spawn_angle: 0.0,
            spawn_x: 0,
            spawn_y: 64,
            spawn_z: 0,
            thundering: false,
            thunder_time: 0,
            version: {
                let mut v = Compound::new();
                v.insert("Id".to_owned(), Value::Int(3715));
                v.insert("Name".to_owned(), Value::String("1.20.1".to_owned()));
                v.insert("Snapshot".to_owned(), Value::Byte(0));
                v.insert("Series".to_owned(), Value::String("main".to_owned()));
                v
            },
            was_modded: false,
        }
    }
}

impl LevelDat {
    /// Creates a new LevelDat with default values.
    pub fn new() -> Self {
        Self {
            data_version: 3715, // MC 1.20.1
            level_data: LevelData::default(),
        }
    }

    /// Creates a new LevelDat with specified level name.
    pub fn with_name(name: impl Into<String>) -> Self {
        let mut level_data = LevelData::default();
        level_data.level_name = name.into();
        Self {
            data_version: 3715,
            level_data,
        }
    }

    /// Reads a LevelDat from a file.
    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        let mut file = File::open(path.as_ref())?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        Self::from_bytes(&buf)
    }

    /// Reads a LevelDat from raw bytes (including GZip header).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut decoder = GzDecoder::new(bytes);
        let mut nbt_buf = Vec::new();
        decoder.read_to_end(&mut nbt_buf)?;

        let mut slice = nbt_buf.as_slice();
        let (compound, _root_name) = from_binary(&mut slice)?;

        Self::from_compound(compound)
    }

    /// Converts from NBT compound.
    fn from_compound(mut compound: Compound) -> Result<Self> {
        let data_version = compound
            .remove("DataVersion")
            .and_then(|v| v.as_int())
            .unwrap_or(3715);

        let data_compound = compound
            .remove("Data")
            .and_then(|v| v.into_compound())
            .ok_or_else(|| LevelDatError::MissingField("Data".to_owned()))?;

        let level_data = Self::parse_level_data(data_compound)?;

        Ok(Self {
            data_version,
            level_data,
        })
    }

    fn parse_level_data(mut compound: Compound) -> Result<LevelData> {
        let get_int = |c: &mut Compound, key: &str, default: i32| -> i32 {
            c.remove(key).and_then(|v| v.as_int()).unwrap_or(default)
        };

        let get_byte = |c: &mut Compound, key: &str, default: i8| -> i8 {
            c.remove(key).and_then(|v| v.as_byte()).unwrap_or(default)
        };

        let get_bool = |c: &mut Compound, key: &str, default: bool| -> bool {
            c.remove(key)
                .and_then(|v| v.as_byte())
                .map(|b| b != 0)
                .unwrap_or(default)
        };

        let get_long = |c: &mut Compound, key: &str, default: i64| -> i64 {
            c.remove(key).and_then(|v| v.as_long()).unwrap_or(default)
        };

        let get_float = |c: &mut Compound, key: &str, default: f32| -> f32 {
            c.remove(key).and_then(|v| v.as_float()).unwrap_or(default)
        };

        let get_double = |c: &mut Compound, key: &str, default: f64| -> f64 {
            c.remove(key).and_then(|v| v.as_double()).unwrap_or(default)
        };

        let get_string = |c: &mut Compound, key: &str, default: &str| -> String {
            c.remove(key)
                .and_then(|v| v.into_string())
                .unwrap_or_else(|| default.to_owned())
        };

        let generator_options = compound
            .remove("generatorOptions")
            .and_then(|v| v.into_compound())
            .unwrap_or_default();

        let version = compound
            .remove("Version")
            .and_then(|v| v.into_compound())
            .unwrap_or_default();

        Ok(LevelData {
            allow_commands: get_bool(&mut compound, "allowCommands", true),
            border_center_x: get_double(&mut compound, "BorderCenterX", 0.0),
            border_center_z: get_double(&mut compound, "BorderCenterZ", 0.0),
            border_damage_per_block: get_double(&mut compound, "BorderDamagePerBlock", 0.2),
            border_danger_zone_time: get_double(&mut compound, "BorderDangerZoneTime", 5.0),
            border_size: get_double(&mut compound, "BorderSize", 60000000.0),
            border_safe_zone: get_double(&mut compound, "BorderSafeZone", 5.0),
            border_teleport_boundary: get_double(
                &mut compound,
                "BorderTeleportBoundary",
                29999984.0,
            ),
            border_warning_blocks: get_double(&mut compound, "BorderWarningBlocks", 5.0),
            border_warning_time: get_double(&mut compound, "BorderWarningTime", 5.0),
            clear_weather_time: get_int(&mut compound, "clearWeatherTime", 0),
            difficulty: get_byte(&mut compound, "Difficulty", 2),
            difficulty_locked: get_bool(&mut compound, "DifficultyLocked", false),
            border_lerp_target: get_double(&mut compound, "BorderLerpTarget", 60000000.0),
            border_lerp_time: get_long(&mut compound, "BorderLerpTime", 0),
            game_type: get_int(&mut compound, "GameType", 0),
            generator_name: get_string(&mut compound, "generatorName", "default"),
            generator_options,
            generator_version: get_int(&mut compound, "generatorVersion", 1),
            hardcoded_spawn_allowed: get_bool(&mut compound, "hardcore", false),
            last_played: get_long(&mut compound, "LastPlayed", 0),
            level_name: get_string(&mut compound, "LevelName", "world"),
            linear_redstone: get_bool(&mut compound, "wasModded", false),
            max_players: get_int(&mut compound, "maxPlayers", 20),
            mob_spawn_range: get_int(&mut compound, "mobSpawnRange", 8),
            player_nether_scale: get_float(&mut compound, "NetherScale", 8.0),
            raining: get_bool(&mut compound, "raining", false),
            rain_time: get_int(&mut compound, "rainTime", 0),
            spawn_angle: get_float(&mut compound, "SpawnAngle", 0.0),
            spawn_x: get_int(&mut compound, "SpawnX", 0),
            spawn_y: get_int(&mut compound, "SpawnY", 64),
            spawn_z: get_int(&mut compound, "SpawnZ", 0),
            thundering: get_bool(&mut compound, "thundering", false),
            thunder_time: get_int(&mut compound, "thunderTime", 0),
            version,
            was_modded: get_bool(&mut compound, "wasModded", false),
        })
    }

    /// Writes this LevelDat to a file.
    pub fn write(&self, path: impl AsRef<Path>) -> Result<()> {
        let bytes = self.to_bytes()?;
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path.as_ref())?;
        file.write_all(&bytes)?;
        Ok(())
    }

    /// Serializes to bytes (with GZip compression).
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let compound = self.to_compound();

        let mut nbt_buf = Vec::new();
        to_binary(&compound, &mut nbt_buf, "")?;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&nbt_buf)?;
        Ok(encoder.finish()?)
    }

    /// Converts to NBT compound.
    fn to_compound(&self) -> Compound {
        let mut root = Compound::new();
        root.insert("DataVersion".to_owned(), Value::Int(self.data_version));

        let mut data = self.level_data.to_compound();
        root.insert("Data".to_owned(), Value::Compound(data));

        root
    }
}

impl LevelData {
    fn to_compound(&self) -> Compound {
        let mut c = Compound::new();

        c.insert(
            "allowCommands".to_owned(),
            Value::Byte(self.allow_commands as i8),
        );
        c.insert(
            "BorderCenterX".to_owned(),
            Value::Double(self.border_center_x),
        );
        c.insert(
            "BorderCenterZ".to_owned(),
            Value::Double(self.border_center_z),
        );
        c.insert(
            "BorderDamagePerBlock".to_owned(),
            Value::Double(self.border_damage_per_block),
        );
        c.insert(
            "BorderDangerZoneTime".to_owned(),
            Value::Double(self.border_danger_zone_time),
        );
        c.insert("BorderSize".to_owned(), Value::Double(self.border_size));
        c.insert(
            "BorderSafeZone".to_owned(),
            Value::Double(self.border_safe_zone),
        );
        c.insert(
            "BorderTeleportBoundary".to_owned(),
            Value::Double(self.border_teleport_boundary),
        );
        c.insert(
            "BorderWarningBlocks".to_owned(),
            Value::Double(self.border_warning_blocks),
        );
        c.insert(
            "BorderWarningTime".to_owned(),
            Value::Double(self.border_warning_time),
        );
        c.insert(
            "clearWeatherTime".to_owned(),
            Value::Int(self.clear_weather_time),
        );
        c.insert("Difficulty".to_owned(), Value::Byte(self.difficulty));
        c.insert(
            "DifficultyLocked".to_owned(),
            Value::Byte(self.difficulty_locked as i8),
        );
        c.insert(
            "BorderLerpTarget".to_owned(),
            Value::Double(self.border_lerp_target),
        );
        c.insert(
            "BorderLerpTime".to_owned(),
            Value::Long(self.border_lerp_time),
        );
        c.insert("GameType".to_owned(), Value::Int(self.game_type));
        c.insert(
            "generatorName".to_owned(),
            Value::String(self.generator_name.clone()),
        );
        c.insert(
            "generatorOptions".to_owned(),
            Value::Compound(self.generator_options.clone()),
        );
        c.insert(
            "generatorVersion".to_owned(),
            Value::Int(self.generator_version),
        );
        c.insert(
            "hardcore".to_owned(),
            Value::Byte(self.hardcoded_spawn_allowed as i8),
        );
        c.insert("LastPlayed".to_owned(), Value::Long(self.last_played));
        c.insert(
            "LevelName".to_owned(),
            Value::String(self.level_name.clone()),
        );
        c.insert("maxPlayers".to_owned(), Value::Int(self.max_players));
        c.insert("mobSpawnRange".to_owned(), Value::Int(self.mob_spawn_range));
        c.insert(
            "NetherScale".to_owned(),
            Value::Float(self.player_nether_scale),
        );
        c.insert("raining".to_owned(), Value::Byte(self.raining as i8));
        c.insert("rainTime".to_owned(), Value::Int(self.rain_time));
        c.insert("SpawnAngle".to_owned(), Value::Float(self.spawn_angle));
        c.insert("SpawnX".to_owned(), Value::Int(self.spawn_x));
        c.insert("SpawnY".to_owned(), Value::Int(self.spawn_y));
        c.insert("SpawnZ".to_owned(), Value::Int(self.spawn_z));
        c.insert("thundering".to_owned(), Value::Byte(self.thundering as i8));
        c.insert("thunderTime".to_owned(), Value::Int(self.thunder_time));
        c.insert("Version".to_owned(), Value::Compound(self.version.clone()));
        c.insert("wasModded".to_owned(), Value::Byte(self.was_modded as i8));

        c
    }
}

impl Default for LevelDat {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_level_dat() {
        let level = LevelDat::new();
        assert_eq!(level.data_version, 3715);
        assert_eq!(level.level_data.spawn_x, 0);
        assert_eq!(level.level_data.spawn_y, 64);
        assert_eq!(level.level_data.spawn_z, 0);
        assert_eq!(level.level_data.game_type, 0);
        assert_eq!(level.level_data.difficulty, 2);
    }

    #[test]
    fn test_roundtrip() {
        let level = LevelDat::new();
        let bytes = level.to_bytes().unwrap();
        let loaded = LevelDat::from_bytes(&bytes).unwrap();

        assert_eq!(loaded.data_version, level.data_version);
        assert_eq!(loaded.level_data.spawn_x, level.level_data.spawn_x);
        assert_eq!(loaded.level_data.spawn_y, level.level_data.spawn_y);
        assert_eq!(loaded.level_data.spawn_z, level.level_data.spawn_z);
        assert_eq!(loaded.level_data.game_type, level.level_data.game_type);
        assert_eq!(loaded.level_data.level_name, level.level_data.level_name);
    }
}
