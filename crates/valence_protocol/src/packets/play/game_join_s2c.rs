use crate::spawn_info::SpawnInfo;
use crate::{Decode, Encode, Packet, VarInt};

#[derive(Clone, Debug, Encode, Decode, Packet)]
pub struct GameJoinS2c {
    pub entity_id: i32,
    pub is_hardcore: bool,
    pub world_names: Vec<String>,
    pub max_players: VarInt,
    pub view_distance: VarInt,
    pub simulation_distance: VarInt,
    pub reduced_debug_info: bool,
    pub enable_respawn_screen: bool,
    pub do_limited_crafting: bool,
    pub world_state: SpawnInfo,
    pub online_mode: bool,
    pub enforces_secure_chat: bool,
}