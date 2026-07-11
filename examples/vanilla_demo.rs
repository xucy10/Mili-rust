#![allow(clippy::type_complexity)]

use valence::prelude::*;
use valence_vanilla::block_update::{set_block_with_update, BlockUpdateEvent, NeighborUpdateEvent};
use valence_vanilla::crop::{Crop, CropPlugin, CropType};
use valence_vanilla::hopper::{Hopper, HopperPlugin};
use valence_vanilla::redstone::signal::RedstoneUpdateQueue;
use valence_vanilla::tick_schedule::TickScheduler;
use valence_vanilla::VanillaPlugin;

const SPAWN_Y: i32 = 64;

pub fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(VanillaPlugin) // 添加原版机制插件
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                init_clients,
                despawn_disconnected_clients,
                place_blocks,
                handle_block_updates,
                handle_neighbor_updates,
            ),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    server: Res<Server>,
    dimensions: Res<DimensionTypeRegistry>,
    biomes: Res<BiomeRegistry>,
) {
    let mut layer = LayerBundle::new(ident!("overworld"), &dimensions, &biomes, &server);

    // 创建一个平坦的草地平台
    for z in -10..10 {
        for x in -10..10 {
            layer.chunk.insert_chunk([x, z], UnloadedChunk::new());
        }
    }

    for z in -50..50 {
        for x in -50..50 {
            layer
                .chunk
                .set_block([x, SPAWN_Y, z], BlockState::GRASS_BLOCK);
        }
    }

    // 放置一些红石示例
    // 红石线
    for x in -3..=3 {
        layer
            .chunk
            .set_block([x, SPAWN_Y + 1, 0], BlockState::REDSTONE_WIRE);
    }

    // 红石火把（信号源）
    layer
        .chunk
        .set_block([-4, SPAWN_Y + 1, 0], BlockState::REDSTONE_TORCH);

    // 红石灯（接收信号）
    layer
        .chunk
        .set_block([4, SPAWN_Y + 1, 0], BlockState::REDSTONE_LAMP);

    // 放置一些耕地和作物示例
    for x in -2..=2 {
        // 耕地
        layer.chunk.set_block([x, SPAWN_Y, 5], BlockState::FARMLAND);

        // 小麦作物
        layer
            .chunk
            .set_block([x, SPAWN_Y + 1, 5], BlockState::WHEAT);
    }

    // 放置漏斗示例
    layer
        .chunk
        .set_block([0, SPAWN_Y + 1, 8], BlockState::HOPPER);

    // 放置箱子
    layer.chunk.set_block([0, SPAWN_Y, 8], BlockState::CHEST);

    commands.spawn(layer);
}

fn init_clients(
    mut clients: Query<&mut Client, Added<Client>>,
    dimensions: Res<DimensionTypeRegistry>,
    biomes: Res<BiomeRegistry>,
    server: Res<Server>,
    mut layers: Query<Entity, With<ChunkLayer>>,
) {
    for mut client in &mut clients {
        let layer_entity = layers.single_mut();
        client.set_player_list((&server).player_list);
        client.teleport(
            IVec3::new(0, SPAWN_Y + 2, 0),
            layers.single(),
            &dimensions,
            &biomes,
            &server,
        );
    }
}

fn place_blocks(
    mut clients: Query<(&mut Inventory, &GameMode, &HeldItem)>,
    mut layers: Query<&mut ChunkLayer>,
    mut events: EventReader<InteractBlockEvent>,
    mut block_events: EventWriter<BlockUpdateEvent>,
    mut neighbor_events: EventWriter<NeighborUpdateEvent>,
) {
    for event in events.read() {
        let Ok((mut inventory, game_mode, held)) = clients.get_mut(event.client) else {
            continue;
        };

        let stack = inventory.slot(held.slot());

        let Some(block_kind) = BlockKind::from_item_kind(stack.item) else {
            continue;
        };

        if *game_mode == GameMode::Survival {
            inventory.set_slot_amount(held.slot(), stack.count - 1);
        }

        let real_pos = event.position.get_in_direction(event.face);

        for mut layer in &mut layers {
            // 使用带更新传播的设置方块函数
            set_block_with_update(
                &mut layer,
                real_pos,
                block_kind.to_state(),
                &mut block_events,
                &mut neighbor_events,
            );
        }
    }
}

fn handle_block_updates(mut events: EventReader<BlockUpdateEvent>) {
    for event in events.read() {
        println!(
            "Block updated at {:?}: {:?} -> {:?}",
            event.position, event.old_state, event.new_state
        );
    }
}

fn handle_neighbor_updates(mut events: EventReader<NeighborUpdateEvent>) {
    for event in events.read() {
        println!(
            "Neighbor update at {:?} from {:?} (direction: {:?})",
            event.position, event.source_position, event.direction
        );
    }
}
