//! Contains dimension types and the dimension type registry. Minecraft's
//! default dimensions are added to the registry by default.
//!
//! ### **NOTE:**
//! - Modifying the dimension type registry after the server has started can
//!   break invariants within instances and clients! Make sure there are no
//!   instances or clients spawned before mutating.

use std::ops::{Deref, DerefMut};

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use serde::{Deserialize, Deserializer, Serialize};
use tracing::error;
use valence_ident::{ident, Ident};
use valence_nbt::serde::CompoundSerializer;

use crate::codec::{RegistryCodec, RegistryValue};
use crate::{Registry, RegistryIdx, RegistrySet};

#[derive(Serialize, Clone, Debug)]
pub struct DimensionType {
    pub ambient_light: f32,
    pub coordinate_scale: f64,
    pub has_ceiling: bool,
    pub has_skylight: bool,
    pub height: i32,
    pub infiniburn: String,
    pub logical_height: i32,
    pub min_y: i32,
    pub monster_spawn_block_light_limit: i32,
    pub monster_spawn_light_level: MonsterSpawnLightLevel,
}

impl<'de> Deserialize<'de> for DimensionType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct DimensionTypeHelper {
            #[serde(default)]
            ambient_light: f32,
            #[serde(default = "default_coordinate_scale")]
            coordinate_scale: f64,
            #[serde(default)]
            has_ceiling: bool,
            #[serde(default)]
            has_skylight: bool,
            #[serde(default = "default_height")]
            height: i32,
            #[serde(default = "default_infiniburn")]
            infiniburn: String,
            #[serde(default = "default_height")]
            logical_height: i32,
            #[serde(default = "default_min_y")]
            min_y: i32,
            #[serde(default)]
            monster_spawn_block_light_limit: i32,
            #[serde(default = "default_monster_spawn")]
            monster_spawn_light_level: MonsterSpawnLightLevel,

            #[serde(default)]
            bed_works: Option<bool>,
            #[serde(default)]
            effects: Option<serde_json::Value>,
            #[serde(default)]
            fixed_time: Option<serde_json::Value>,
            #[serde(default)]
            has_raids: Option<bool>,
            #[serde(default)]
            natural: Option<bool>,
            #[serde(default)]
            piglin_safe: Option<bool>,
            #[serde(default)]
            respawn_anchor_works: Option<bool>,
            #[serde(default)]
            ultrawarm: Option<bool>,
            #[serde(default)]
            has_fixed_time: Option<bool>,
            #[serde(default)]
            has_ender_dragon_fight: Option<bool>,
            #[serde(default)]
            default_clock: Option<String>,
            #[serde(default)]
            timelines: Option<serde_json::Value>,
            #[serde(default)]
            skybox: Option<String>,
            #[serde(default)]
            cardinal_light: Option<String>,
            #[serde(default)]
            attributes: Option<serde_json::Value>,
        }

        fn default_coordinate_scale() -> f64 {
            1.0
        }
        fn default_height() -> i32 {
            384
        }
        fn default_min_y() -> i32 {
            -64
        }
        fn default_infiniburn() -> String {
            "#minecraft:infiniburn_overworld".into()
        }
        fn default_monster_spawn() -> MonsterSpawnLightLevel {
            MonsterSpawnLightLevel::Int(7)
        }

        let h = DimensionTypeHelper::deserialize(deserializer)?;

        Ok(DimensionType {
            ambient_light: h.ambient_light,
            coordinate_scale: h.coordinate_scale,
            has_ceiling: h.has_ceiling,
            has_skylight: h.has_skylight,
            height: h.height,
            infiniburn: h.infiniburn,
            logical_height: h.logical_height,
            min_y: h.min_y,
            monster_spawn_block_light_limit: h.monster_spawn_block_light_limit,
            monster_spawn_light_level: h.monster_spawn_light_level,
        })
    }
}
pub struct DimensionTypePlugin;

impl Plugin for DimensionTypePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DimensionTypeRegistry>()
            .add_systems(PreStartup, load_default_dimension_types)
            .add_systems(
                PostUpdate,
                update_dimension_type_registry.before(RegistrySet),
            );
    }
}

/// Loads the default dimension types from the registry codec.
fn load_default_dimension_types(mut reg: ResMut<DimensionTypeRegistry>, codec: Res<RegistryCodec>) {
    let mut helper = move || -> anyhow::Result<()> {
        for value in codec.registry(DimensionTypeRegistry::KEY) {
            let mut dimension_type = DimensionType::deserialize(value.element.clone())?;

            // HACK: We don't have a lighting engine implemented. To avoid shrouding the
            // world in darkness, give all dimensions the max ambient light.
            dimension_type.ambient_light = 1.0;

            reg.insert(value.name.clone(), dimension_type);
        }

        Ok(())
    };

    if let Err(e) = helper() {
        error!("failed to load default dimension types from registry codec: {e:#}");
    }
}

/// Updates the registry codec as the dimension type registry is modified by
/// users.
fn update_dimension_type_registry(
    reg: Res<DimensionTypeRegistry>,
    mut codec: ResMut<RegistryCodec>,
) {
    if reg.is_changed() {
        let dimension_types = codec.registry_mut(DimensionTypeRegistry::KEY);

        dimension_types.clear();

        dimension_types.extend(reg.iter().map(|(_, name, dim)| {
            RegistryValue {
                name: name.into(),
                element: dim
                    .serialize(CompoundSerializer)
                    .expect("failed to serialize dimension type"),
            }
        }));
    }
}

#[derive(Resource, Default, Debug)]
pub struct DimensionTypeRegistry {
    reg: Registry<DimensionTypeId, DimensionType>,
}

impl DimensionTypeRegistry {
    pub const KEY: Ident<&'static str> = ident!("dimension_type");
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug)]
pub struct DimensionTypeId(u16);

impl RegistryIdx for DimensionTypeId {
    const MAX: usize = u16::MAX as usize;

    fn to_index(self) -> usize {
        self.0 as usize
    }

    fn from_index(idx: usize) -> Self {
        Self(idx as u16)
    }
}

impl Deref for DimensionTypeRegistry {
    type Target = Registry<DimensionTypeId, DimensionType>;

    fn deref(&self) -> &Self::Target {
        &self.reg
    }
}

impl DerefMut for DimensionTypeRegistry {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.reg
    }
}

impl Default for DimensionType {
    fn default() -> Self {
        Self {
            ambient_light: 0.0,
            coordinate_scale: 1.0,
            has_ceiling: false,
            has_skylight: true,
            height: 384,
            infiniburn: "#minecraft:infiniburn_overworld".into(),
            logical_height: 384,
            min_y: -64,
            monster_spawn_block_light_limit: 0,
            monster_spawn_light_level: MonsterSpawnLightLevel::Int(7),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MonsterSpawnLightLevel {
    Int(i32),
    Tagged(MonsterSpawnLightLevelTagged),
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MonsterSpawnLightLevelTagged {
    #[serde(rename = "minecraft:uniform")]
    Uniform {
        min_inclusive: i32,
        max_inclusive: i32,
    },
}

impl From<i32> for MonsterSpawnLightLevel {
    fn from(value: i32) -> Self {
        Self::Int(value)
    }
}
