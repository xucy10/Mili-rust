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
