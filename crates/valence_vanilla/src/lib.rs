#![allow(
    clippy::cast_lossless,
    clippy::doc_markdown,
    clippy::derivable_impls,
    clippy::new_without_default,
    clippy::needless_continue,
    clippy::used_underscore_binding,
    clippy::match_wildcard_for_single_variants,
    clippy::for_kv_map,
    clippy::unnecessary_cast,
    clippy::map_unwrap_or,
    clippy::manual_find,
    clippy::explicit_auto_deref,
    clippy::unseparated_literal_suffix,
    unused_mut
)]

pub mod block_update;
pub mod crop;
pub mod entity_ai;
pub mod hopper;
pub mod physics;
pub mod redstone;
pub mod tick_schedule;
pub mod villager;

use bevy_app::prelude::*;

pub struct VanillaPlugin;

impl Plugin for VanillaPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            block_update::BlockUpdatePlugin,
            tick_schedule::TickSchedulePlugin,
            hopper::HopperPlugin,
            crop::CropPlugin,
            redstone::RedstonePlugin,
            villager::VillagerPlugin,
            physics::PhysicsPlugin,
            entity_ai::EntityAiPlugin,
        ));
    }
}
