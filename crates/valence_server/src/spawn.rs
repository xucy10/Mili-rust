//! Handles spawning and respawning the client.

use std::borrow::Cow;

use bevy_ecs::prelude::*;
use bevy_ecs::query::QueryData;
use derive_more::{Deref, DerefMut};
use valence_entity::EntityLayerId;
use valence_protocol::packets::play::{
    GameEventKind, GameJoinS2c, GameStateChangeS2c, PlayerRespawnS2c, PlayerSpawnPositionS2c,
};
use valence_protocol::{BlockPos, GameMode, GlobalPos, Ident, SpawnInfo, VarInt, WritePacket};
use valence_registry::tags::TagsRegistry;
use valence_registry::{DimensionTypeRegistry, RegistryIdx};

use crate::client::{Client, ViewDistance, VisibleChunkLayer};
use crate::layer::ChunkLayer;

// Components for the join game and respawn packet.

#[derive(Component, Clone, PartialEq, Eq, Default, Debug)]
pub struct DeathLocation(pub Option<(Ident<String>, BlockPos)>);

#[derive(Component, Copy, Clone, PartialEq, Eq, Default, Debug, Deref, DerefMut)]
pub struct IsHardcore(pub bool);

/// Hashed world seed used for biome noise.
#[derive(Component, Copy, Clone, PartialEq, Eq, Default, Debug, Deref, DerefMut)]
pub struct HashedSeed(pub u64);

#[derive(Component, Copy, Clone, PartialEq, Eq, Default, Debug, Deref, DerefMut)]
pub struct ReducedDebugInfo(pub bool);

#[derive(Component, Copy, Clone, PartialEq, Eq, Debug, Deref, DerefMut)]
pub struct HasRespawnScreen(pub bool);

/// If the client is spawning into a debug world.
#[derive(Component, Copy, Clone, PartialEq, Eq, Default, Debug, Deref, DerefMut)]
pub struct IsDebug(pub bool);

/// Changes the perceived horizon line (used for superflat worlds).
#[derive(Component, Copy, Clone, PartialEq, Eq, Default, Debug, Deref, DerefMut)]
pub struct IsFlat(pub bool);

#[derive(Component, Copy, Clone, PartialEq, Eq, Default, Debug, Deref, DerefMut)]
pub struct PortalCooldown(pub i32);

/// The initial previous gamemode. Used for the F3+F4 gamemode switcher.
#[derive(Component, Copy, Clone, PartialEq, Eq, Default, Debug, Deref, DerefMut)]
pub struct PrevGameMode(pub Option<GameMode>);

impl Default for HasRespawnScreen {
    fn default() -> Self {
        Self(true)
    }
}

/// The position and angle that clients will respawn with. Also
/// controls the position that compasses point towards.
#[derive(Component, Copy, Clone, PartialEq, Default, Debug)]
pub struct RespawnPosition {
    /// The position that clients will respawn at. This can be changed at any
    /// time to set the position that compasses point towards.
    pub pos: BlockPos,
    /// The yaw angle that clients will respawn with (in degrees).
    pub yaw: f32,
}

/// A convenient [`QueryData`] for obtaining client spawn components. Also see
/// [`ClientSpawnQueryReadOnly`].
#[derive(QueryData)]
#[query_data(mutable)]
pub struct ClientSpawnQuery {
    pub is_hardcore: &'static mut IsHardcore,
    pub game_mode: &'static mut GameMode,
    pub prev_game_mode: &'static mut PrevGameMode,
    pub hashed_seed: &'static mut HashedSeed,
    pub view_distance: &'static mut ViewDistance,
    pub reduced_debug_info: &'static mut ReducedDebugInfo,
    pub has_respawn_screen: &'static mut HasRespawnScreen,
    pub is_debug: &'static mut IsDebug,
    pub is_flat: &'static mut IsFlat,
    pub death_loc: &'static mut DeathLocation,
    pub portal_cooldown: &'static mut PortalCooldown,
}

fn sea_level_for_dimension(dim_name: &str) -> i32 {
    if dim_name.contains("overworld") {
        63
    } else if dim_name.contains("nether") {
        32
    } else if dim_name.contains("end") {
        0
    } else {
        63
    }
}

pub(super) fn initial_join(
    dim_type_reg: Res<DimensionTypeRegistry>,
    tags: Res<TagsRegistry>,
    mut clients: Query<(&mut Client, &VisibleChunkLayer, ClientSpawnQueryReadOnly), Added<Client>>,
    chunk_layers: Query<&ChunkLayer>,
) {
    for (mut client, visible_chunk_layer, spawn) in &mut clients {
        let Ok(chunk_layer) = chunk_layers.get(visible_chunk_layer.0) else {
            continue;
        };

        let dimension_type_name = chunk_layer.dimension_type_name();

        eprintln!("[DEBUG] === GAME JOIN INFO ===");
        eprintln!("[DEBUG] dimension_type_name: {}", dimension_type_name);
        eprintln!("[DEBUG] =========================");

        let dim_type_id = dim_type_reg
            .index_of(dimension_type_name)
            .map(|id| VarInt(id.to_index() as i32))
            .unwrap_or(VarInt(0));

        let sea_level = sea_level_for_dimension(dimension_type_name.as_str());

        let last_death_location = spawn.death_loc.0.as_ref().map(|(id, pos)| GlobalPos {
            dimension_name: id.as_str_ident().into(),
            position: *pos,
        });

        // 🔍 DEBUG: Check if death location is causing issues
        eprintln!("[DEBUG] === SPAWN INFO ANALYSIS ===");
        eprintln!("[DEBUG] death_location: {:?}", spawn.death_loc.0);
        eprintln!("[DEBUG] last_death_location (GlobalPos): {:?}", last_death_location);
        if let Some(ref pos) = last_death_location {
            eprintln!("[DEBUG]   dimension_name: {}", pos.dimension_name);
            eprintln!("[DEBUG]   position: {:?}", pos.position);
        }
        eprintln!("[DEBUG] ==============================");

        // Build the complete 26.2 compliant GameJoinS2c packet
        let world_state = SpawnInfo {
            dimension: dim_type_id.clone(),
            name: dimension_type_name.to_string(),
            hashed_seed: 0,
            gamemode: 0, // survival
            previous_gamemode: -1, // not set
            is_debug: false,
            is_flat: false,
            death_location: last_death_location.map(|pos| valence_protocol::spawn_info::GlobalPos {
                dimension_name: pos.dimension_name.to_string(),
                x: pos.position.x,
                y: pos.position.y,
                z: pos.position.z,
            }),
            portal_cooldown: VarInt(0),
            sea_level: VarInt(64),
        };

        let join_packet = GameJoinS2c {
            entity_id: 1,
            is_hardcore: spawn.is_hardcore.0,
            world_names: vec![dimension_type_name.to_string()],
            max_players: VarInt(20),
            view_distance: VarInt(i32::from(spawn.view_distance.get())),
            simulation_distance: VarInt(10),
            reduced_debug_info: false,
            enable_respawn_screen: true,
            do_limited_crafting: false,
            world_state,
            online_mode: true,
            enforces_secure_chat: false,
        };

        _ = client.enc.prepend_packet(&join_packet);

        _ = client.write_packet(&GameStateChangeS2c {
            kind: GameEventKind::StartWaitingForLevelChunks,
            value: 0.0,
        });
    }
}

pub(super) fn respawn(
    dim_type_reg: Res<DimensionTypeRegistry>,
    mut clients: Query<
        (
            &mut Client,
            &EntityLayerId,
            &DeathLocation,
            &HashedSeed,
            &GameMode,
            &PrevGameMode,
            &IsDebug,
            &IsFlat,
        ),
        Changed<VisibleChunkLayer>,
    >,
    chunk_layers: Query<&ChunkLayer>,
) {
    for (mut client, loc, death_loc, hashed_seed, game_mode, prev_game_mode, is_debug, is_flat) in
        &mut clients
    {
        if client.is_added() {
            continue;
        }

        let Ok(chunk_layer) = chunk_layers.get(loc.0) else {
            continue;
        };

        let dimension_type_name = chunk_layer.dimension_type_name();

        let dim_type_id = dim_type_reg
            .index_of(dimension_type_name)
            .map(|id| VarInt(id.to_index() as i32))
            .unwrap_or(VarInt(0));

        let sea_level = sea_level_for_dimension(dimension_type_name.as_str());

        let last_death_location = death_loc.0.as_ref().map(|(id, pos)| GlobalPos {
            dimension_name: id.as_str_ident().into(),
            position: *pos,
        });

        let world_state = SpawnInfo {
            dimension: dim_type_id.clone(),
            name: dimension_type_name.to_string(),
            hashed_seed: 0,
            gamemode: 0,
            previous_gamemode: -1,
            is_debug: false,
            is_flat: false,
            death_location: None,
            portal_cooldown: VarInt(0),
            sea_level: VarInt(64),
        };

        client.write_packet(&PlayerRespawnS2c {
            world_state,
            copy_metadata: 1,
        });
    }
}

/// Sets the client's respawn and compass position.
///
/// This also closes the "downloading terrain" screen when first joining, so
/// it should happen after the initial chunks are written.
pub(super) fn update_respawn_position(
    mut clients: Query<(&mut Client, &RespawnPosition), Changed<RespawnPosition>>,
) {
    for (mut client, respawn_pos) in &mut clients {
        client.write_packet(&PlayerSpawnPositionS2c {
            dimension_name: Ident::new(Cow::Borrowed("minecraft:overworld")).unwrap(),
            position: u64::from(respawn_pos.pos.packed().unwrap()) as i64,
            yaw: respawn_pos.yaw,
            pitch: 0.0,
        });
    }
}