use crate::spawn_info::SpawnInfo;
use crate::{Decode, Encode, Packet};

#[derive(Clone, PartialEq, Debug, Encode, Decode, Packet)]
pub struct PlayerRespawnS2c {
    pub world_state: SpawnInfo,
    pub copy_metadata: u8,
}