use std::borrow::Cow;

use crate::{Decode, Encode, Packet, PacketState};

#[derive(Clone, Debug, Encode, Decode, Packet)]
#[packet(id = 7, state = PacketState::Configuration)]
pub struct ConfigSelectKnownPacksC2s<'a> {
    pub packs: Cow<'a, [KnownPack<'a>]>,
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct KnownPack<'a> {
    pub namespace: &'a str,
    pub id: &'a str,
    pub version: &'a str,
}
