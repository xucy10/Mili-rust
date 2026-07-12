use std::borrow::Cow;

use valence_ident::Ident;

use crate::{Bounded, Decode, Encode, Packet, PacketState, RawBytes};

const MAX_PAYLOAD_SIZE: usize = 0x100000;

#[derive(Clone, Debug, Encode, Decode, Packet)]
#[packet(id = 1, state = PacketState::Configuration)]
pub struct ConfigCustomPayloadS2c<'a> {
    pub channel: Ident<Cow<'a, str>>,
    pub data: Bounded<RawBytes<'a>, MAX_PAYLOAD_SIZE>,
}
