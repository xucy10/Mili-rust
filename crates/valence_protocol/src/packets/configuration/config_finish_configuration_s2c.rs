use std::borrow::Cow;

use crate::{Decode, Encode, Packet, PacketState};

#[derive(Clone, Debug, Encode, Decode, Packet)]
#[packet(id = 3, state = PacketState::Configuration)]
pub struct ConfigFinishConfigurationS2c;
