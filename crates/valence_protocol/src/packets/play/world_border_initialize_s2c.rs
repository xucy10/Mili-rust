use std::io::Write;

use crate::{Encode, Packet, PacketSide, PacketState};

#[derive(Copy, Clone, Debug, Default)]
pub struct WorldBorderInitializeS2c;

impl Packet for WorldBorderInitializeS2c {
    const ID: i32 = 43;
    const NAME: &'static str = "WorldBorderInitializeS2c_STUB";
    const SIDE: PacketSide = PacketSide::Clientbound;
    const STATE: PacketState = PacketState::Play;
}

impl Encode for WorldBorderInitializeS2c {
    fn encode(&self, _w: impl Write) -> anyhow::Result<()> {
        Ok(())
    }
}