use std::borrow::Cow;

use crate::{Decode, Encode, Packet, PacketState};

#[derive(Clone, Debug, Encode, Decode, Packet)]
#[packet(id = 14, state = PacketState::Configuration)]
pub struct ConfigSelectKnownPacksS2c<'a> {
    pub packs: Cow<'a, [KnownPack<'a>]>,
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct KnownPack<'a> {
    pub namespace: &'a str,
    pub id: &'a str,
    pub version: &'a str,
}
