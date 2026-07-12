#![allow(clippy::type_complexity)]

use valence::prelude::*;
use valence_vanilla::block_update::{BlockUpdateEvent, NeighborUpdateEvent};
use valence_vanilla::VanillaPlugin;

const SPAWN_Y: i32 = 64;

pub fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(VanillaPlugin)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                init_clients,
                despawn_disconnected_clients,
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

    for x in -3..=3 {
        layer
            .chunk
            .set_block([x, SPAWN_Y + 1, 0], BlockState::REDSTONE_WIRE);
    }

    layer
        .chunk
        .set_block([-4, SPAWN_Y + 1, 0], BlockState::REDSTONE_TORCH);

    layer
        .chunk
        .set_block([4, SPAWN_Y + 1, 0], BlockState::REDSTONE_LAMP);

    for x in -2..=2 {
        layer.chunk.set_block([x, SPAWN_Y, 5], BlockState::FARMLAND);
        layer
            .chunk
            .set_block([x, SPAWN_Y + 1, 5], BlockState::WHEAT);
    }

    layer
        .chunk
        .set_block([0, SPAWN_Y + 1, 8], BlockState::HOPPER);

    layer.chunk.set_block([0, SPAWN_Y, 8], BlockState::CHEST);

    commands.spawn(layer);
}

fn init_clients(
    mut clients: Query<
        (
            &mut EntityLayerId,
            &mut VisibleChunkLayer,
            &mut VisibleEntityLayers,
            &mut Position,
        ),
        Added<Client>,
    >,
    layers: Query<Entity, (With<ChunkLayer>, With<EntityLayer>)>,
) {
    for (mut layer_id, mut visible_chunk_layer, mut visible_entity_layers, mut pos) in &mut clients
    {
        let layer = layers.single();

        layer_id.0 = layer;
        visible_chunk_layer.0 = layer;
        visible_entity_layers.0.insert(layer);
        pos.set([0.0, f64::from(SPAWN_Y + 2), 0.0]);
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
