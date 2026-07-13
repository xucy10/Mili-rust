//! Contains biomes and the biome registry. Minecraft's default biomes are added
//! to the registry by default.
//!
//! ### **NOTE:**
//! - Modifying the biome registry after the server has started can break
//!   invariants within instances and clients! Make sure there are no instances
//!   or clients spawned before mutating.
//! - A biome named "minecraft:plains" must exist. Otherwise, vanilla clients
//!   will be disconnected.

use std::ops::{Deref, DerefMut};

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use serde::{Deserialize, Deserializer, Serialize};
use tracing::error;
use valence_ident::{ident, Ident};
use valence_nbt::serde::CompoundSerializer;

use crate::codec::{RegistryCodec, RegistryValue};
use crate::{Registry, RegistryIdx, RegistrySet};

pub struct BiomePlugin;

impl Plugin for BiomePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BiomeRegistry>()
            .add_systems(PreStartup, load_default_biomes)
            .add_systems(PostUpdate, update_biome_registry.before(RegistrySet));
    }
}

fn load_default_biomes(mut reg: ResMut<BiomeRegistry>, codec: Res<RegistryCodec>) {
    let mut helper = move || -> anyhow::Result<()> {
        for value in codec.registry(BiomeRegistry::KEY) {
            let biome = Biome::deserialize(value.element.clone())?;

            reg.insert(value.name.clone(), biome);
        }

        // Move "plains" to the front so that `BiomeId::default()` is the ID of plains.
        reg.swap_to_front(ident!("plains"));

        Ok(())
    };

    if let Err(e) = helper() {
        error!("failed to load default biomes from registry codec: {e:#}");
    }
}

fn update_biome_registry(reg: Res<BiomeRegistry>, mut codec: ResMut<RegistryCodec>) {
    if reg.is_changed() {
        let biomes = codec.registry_mut(BiomeRegistry::KEY);

        biomes.clear();

        biomes.extend(reg.iter().map(|(_, name, biome)| {
            RegistryValue {
                name: name.into(),
                element: biome
                    .serialize(CompoundSerializer)
                    .expect("failed to serialize biome"),
            }
        }));
    }
}

#[derive(Resource, Default, Debug)]
pub struct BiomeRegistry {
    reg: Registry<BiomeId, Biome>,
}

impl BiomeRegistry {
    pub const KEY: Ident<&'static str> = ident!("worldgen/biome");
}

impl Deref for BiomeRegistry {
    type Target = Registry<BiomeId, Biome>;

    fn deref(&self) -> &Self::Target {
        &self.reg
    }
}

impl DerefMut for BiomeRegistry {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.reg
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug)]
pub struct BiomeId(u32);

impl BiomeId {
    pub const DEFAULT: Self = BiomeId(0);
}

impl RegistryIdx for BiomeId {
    const MAX: usize = u32::MAX as usize;

    #[inline]
    fn to_index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    fn from_index(idx: usize) -> Self {
        Self(idx as u32)
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct Biome {
    pub downfall: f32,
    pub effects: BiomeEffects,
    pub has_precipitation: bool,
    pub temperature: f32,
}

impl<'de> Deserialize<'de> for Biome {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct BiomeHelper {
            #[serde(default)]
            downfall: f32,
            effects: BiomeEffects,
            #[serde(default = "default_has_precipitation")]
            has_precipitation: bool,
            #[serde(default = "default_temperature")]
            temperature: f32,
            #[serde(default)]
            attributes: Option<serde_json::Value>,
            #[serde(default)]
            carvers: Option<serde_json::Value>,
            #[serde(default)]
            features: Option<serde_json::Value>,
            #[serde(default)]
            spawners: Option<serde_json::Value>,
            #[serde(default)]
            spawn_costs: Option<serde_json::Value>,
        }

        fn default_has_precipitation() -> bool {
            true
        }
        fn default_temperature() -> f32 {
            0.8
        }

        let h = BiomeHelper::deserialize(deserializer)?;
        Ok(Biome {
            downfall: h.downfall,
            effects: h.effects,
            has_precipitation: h.has_precipitation,
            temperature: h.temperature,
        })
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct BiomeEffects {
    pub fog_color: u32,
    pub sky_color: u32,
    pub water_color: u32,
    pub water_fog_color: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grass_color: Option<u32>,
}

fn parse_color<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    use serde::de;

    struct ColorVisitor;

    impl<'de> de::Visitor<'de> for ColorVisitor {
        type Value = u32;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a u32 integer or a hex color string like \"#aabbcc\"")
        }

        fn visit_u32<E: de::Error>(self, v: u32) -> Result<u32, E> {
            Ok(v)
        }

        fn visit_i32<E: de::Error>(self, v: i32) -> Result<u32, E> {
            Ok(v as u32)
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> Result<u32, E> {
            Ok(v as u32)
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> Result<u32, E> {
            Ok(v as u32)
        }

        fn visit_f64<E: de::Error>(self, v: f64) -> Result<u32, E> {
            Ok(v as u32)
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<u32, E> {
            parse_hex_color(v).map_err(de::Error::custom)
        }

        fn visit_string<E: de::Error>(self, v: String) -> Result<u32, E> {
            parse_hex_color(&v).map_err(de::Error::custom)
        }
    }

    deserializer.deserialize_any(ColorVisitor)
}

fn parse_hex_color(s: &str) -> Result<u32, String> {
    let s = s.trim_start_matches('#');
    if s.len() == 6 {
        let r = u32::from_str_radix(&s[0..2], 16).map_err(|e| format!("bad red: {e}"))?;
        let g = u32::from_str_radix(&s[2..4], 16).map_err(|e| format!("bad green: {e}"))?;
        let b = u32::from_str_radix(&s[4..6], 16).map_err(|e| format!("bad blue: {e}"))?;
        Ok((r << 16) | (g << 8) | b)
    } else if s.len() == 8 {
        let a = u32::from_str_radix(&s[0..2], 16).map_err(|e| format!("bad alpha: {e}"))?;
        let r = u32::from_str_radix(&s[2..4], 16).map_err(|e| format!("bad red: {e}"))?;
        let g = u32::from_str_radix(&s[4..6], 16).map_err(|e| format!("bad green: {e}"))?;
        let b = u32::from_str_radix(&s[6..8], 16).map_err(|e| format!("bad blue: {e}"))?;
        Ok((a << 24) | (r << 16) | (g << 8) | b)
    } else {
        Err(format!("invalid hex color: {s:?}"))
    }
}

impl<'de> Deserialize<'de> for BiomeEffects {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct BiomeEffectsHelper {
            #[serde(deserialize_with = "parse_color")]
            fog_color: u32,
            #[serde(deserialize_with = "parse_color")]
            sky_color: u32,
            #[serde(deserialize_with = "parse_color")]
            water_color: u32,
            #[serde(deserialize_with = "parse_color")]
            water_fog_color: u32,
            #[serde(default, deserialize_with = "parse_color_opt")]
            grass_color: Option<u32>,
            #[serde(default)]
            foliage_color: Option<serde_json::Value>,
            #[serde(default)]
            grass_color_modifier: Option<serde_json::Value>,
            #[serde(default)]
            mood_sound: Option<serde_json::Value>,
            #[serde(default)]
            additions_sound: Option<serde_json::Value>,
            #[serde(default)]
            ambient_sound: Option<serde_json::Value>,
            #[serde(default)]
            music: Option<serde_json::Value>,
            #[serde(default)]
            music_volume: Option<f32>,
            #[serde(default)]
            particle: Option<serde_json::Value>,
            #[serde(default)]
            dry_foliage_color: Option<serde_json::Value>,
        }

        let h = BiomeEffectsHelper::deserialize(deserializer)?;
        Ok(BiomeEffects {
            fog_color: h.fog_color,
            sky_color: h.sky_color,
            water_color: h.water_color,
            water_fog_color: h.water_fog_color,
            grass_color: h.grass_color,
        })
    }
}

fn parse_color_opt<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u32>, D::Error> {
    use serde::de;

    struct OptColorVisitor;

    impl<'de> de::Visitor<'de> for OptColorVisitor {
        type Value = Option<u32>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("null, a u32 integer, or a hex color string")
        }

        fn visit_bool<E: de::Error>(self, _v: bool) -> Result<Option<u32>, E> {
            Ok(None)
        }

        fn visit_u32<E: de::Error>(self, v: u32) -> Result<Option<u32>, E> {
            Ok(Some(v))
        }

        fn visit_i32<E: de::Error>(self, v: i32) -> Result<Option<u32>, E> {
            Ok(Some(v as u32))
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> Result<Option<u32>, E> {
            Ok(Some(v as u32))
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> Result<Option<u32>, E> {
            Ok(Some(v as u32))
        }

        fn visit_f64<E: de::Error>(self, _v: f64) -> Result<Option<u32>, E> {
            Ok(None)
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Option<u32>, E> {
            Ok(Some(parse_hex_color(v).map_err(de::Error::custom)?))
        }

        fn visit_string<E: de::Error>(self, v: String) -> Result<Option<u32>, E> {
            Ok(Some(parse_hex_color(&v).map_err(de::Error::custom)?))
        }

        fn visit_none<E: de::Error>(self) -> Result<Option<u32>, E> {
            Ok(None)
        }

        fn visit_unit<E: de::Error>(self) -> Result<Option<u32>, E> {
            Ok(None)
        }
    }

    deserializer.deserialize_any(OptColorVisitor)
}

impl Default for Biome {
    fn default() -> Self {
        Self {
            downfall: 0.4,
            effects: BiomeEffects::default(),
            has_precipitation: true,
            temperature: 0.8,
        }
    }
}

impl Default for BiomeEffects {
    fn default() -> Self {
        Self {
            fog_color: 12638463,
            sky_color: 7907327,
            water_color: 4159204,
            water_fog_color: 329011,
            grass_color: None,
        }
    }
}
