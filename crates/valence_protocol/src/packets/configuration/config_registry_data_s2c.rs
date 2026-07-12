use std::borrow::Cow;

use valence_ident::Ident;
use valence_nbt::Compound;

use crate::{Decode, Encode, Packet, PacketState, VarInt};

#[derive(Clone, Debug, Encode, Decode, Packet)]
#[packet(id = 7, state = PacketState::Configuration)]
pub struct ConfigRegistryDataS2c<'a> {
    pub registry_id: Ident<Cow<'a, str>>,
    pub entries: Cow<'a, [RegistryEntry<'a>]>,
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct RegistryEntry<'a> {
    pub entry_id: Ident<Cow<'a, str>>,
    pub data: Option<Compound>,
}
