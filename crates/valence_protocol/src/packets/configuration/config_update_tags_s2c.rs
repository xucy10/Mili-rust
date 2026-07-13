use std::borrow::Cow;
use std::collections::BTreeMap;

use valence_ident::Ident;

use crate::{Decode, Encode, Packet, PacketState};

#[derive(Clone, Debug, Encode, Decode, Packet)]
#[packet(id = 13, state = PacketState::Configuration)]
pub struct ConfigUpdateTagsS2c<'a> {
    pub groups: Cow<'a, ConfigRegistryMap>,
}

pub type ConfigRegistryMap = BTreeMap<Ident<String>, BTreeMap<Ident<String>, Vec<Ident<String>>>>;