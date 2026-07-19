use std::borrow::Cow;

use valence_ident::Ident;

use crate::{Decode, Encode, Packet};

#[derive(Clone, Debug, Encode, Decode, Packet)]
pub struct PlayerSpawnPositionS2c<'a> {
    pub dimension_name: Ident<Cow<'a, str>>,
    pub position: i64,
    pub yaw: f32,
    pub pitch: f32,
}
