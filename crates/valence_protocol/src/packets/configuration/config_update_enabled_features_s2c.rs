use std::borrow::Cow;

use crate::{Decode, Encode, Packet, PacketState};

#[derive(Clone, Debug, Encode, Decode, Packet)]
#[packet(id = 12, state = PacketState::Configuration)]
pub struct ConfigUpdateEnabledFeaturesS2c<'a> {
    pub features: Cow<'a, [&'a str]>,
}
