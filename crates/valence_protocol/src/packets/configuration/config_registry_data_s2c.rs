use std::borrow::Cow;
use std::io::Write;

use valence_ident::Ident;
use valence_nbt::Compound;

use crate::{Decode, Encode, Packet, PacketState};

#[derive(Clone, Debug, Encode, Decode, Packet)]
#[packet(id = 7, state = PacketState::Configuration)]
pub struct ConfigRegistryDataS2c<'a> {
    pub registry_id: Ident<Cow<'a, str>>,
    pub entries: Cow<'a, [RegistryEntry<'a>]>,
}

#[derive(Clone, Debug)]
pub struct RegistryEntry<'a> {
    pub entry_id: Ident<Cow<'a, str>>,
    pub data: Option<Compound>,
}

impl Encode for RegistryEntry<'_> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.entry_id.encode(&mut w)?;
        match &self.data {
            Some(c) => c.encode(&mut w),
            None => Ok(w.write_all(&[0])?),
        }
    }
}

impl<'a> Decode<'a> for RegistryEntry<'a> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let entry_id = Ident::decode(r)?;
        let data = if r.first() == Some(&0) {
            *r = &r[1..];
            None
        } else {
            Some(Compound::decode(r)?)
        };
        Ok(RegistryEntry { entry_id, data })
    }
}
