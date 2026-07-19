use valence_math::DVec3;

use crate::{Decode, Encode, Packet, VarInt};

#[derive(Copy, Clone, PartialEq, Debug, Encode, Decode, Packet)]
pub struct PlayerPositionLookS2c {
    pub teleport_id: VarInt,
    pub position: DVec3,
    pub velocity: DVec3,
    pub yaw: f32,
    pub pitch: f32,
    pub flags: i32,
}
