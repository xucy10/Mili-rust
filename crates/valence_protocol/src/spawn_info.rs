use std::io::Write;

use anyhow::Context;

use crate::{Decode, Encode, VarInt};

fn pack_position(x: i32, y: i32, z: i32) -> i64 {
    ((x as i64 & 0x3FFFFFF) << 38) | ((z as i64 & 0x3FFFFFF) << 12) | (y as i64 & 0xFFF)
}

fn unpack_position(val: i64) -> (i32, i32, i32) {
    let x = (val >> 38) as i32;
    let y = (val << 52 >> 52) as i32;
    let z = (val << 26 >> 38) as i32;
    (x, y, z)
}

#[derive(Clone, PartialEq, Debug)]
pub struct GlobalPos {
    pub dimension_name: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Encode for GlobalPos {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.dimension_name.encode(&mut w).context("failed to encode dimension_name")?;
        let packed = pack_position(self.x, self.y, self.z);
        packed.encode(&mut w).context("failed to encode packed position")?;
        Ok(())
    }
}

impl<'a> Decode<'a> for GlobalPos {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let dimension_name = String::decode(r).context("failed to decode dimension_name")?;
        let packed = i64::decode(r).context("failed to decode packed position")?;
        let (x, y, z) = unpack_position(packed);

        Ok(Self {
            dimension_name,
            x,
            y,
            z,
        })
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct SpawnInfo {
    pub dimension: VarInt,
    pub name: String,
    pub hashed_seed: i64,
    pub gamemode: u8,
    pub previous_gamemode: i8,
    pub is_debug: bool,
    pub is_flat: bool,
    pub death_location: Option<GlobalPos>,
    pub portal_cooldown: VarInt,
    pub sea_level: VarInt,
}

impl Encode for SpawnInfo {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.dimension.encode(&mut w).context("failed to encode dimension")?;
        self.name.encode(&mut w).context("failed to encode name")?;
        self.hashed_seed.encode(&mut w).context("failed to encode hashed_seed")?;
        self.gamemode.encode(&mut w).context("failed to encode gamemode")?;
        self.previous_gamemode.encode(&mut w).context("failed to encode previous_gamemode")?;
        self.is_debug.encode(&mut w).context("failed to encode is_debug")?;
        self.is_flat.encode(&mut w).context("failed to encode is_flat")?;
        
        match &self.death_location {
            Some(pos) => {
                true.encode(&mut w).context("failed to encode death_location present")?;
                pos.encode(&mut w).context("failed to encode death_location")?;
            }
            None => {
                false.encode(&mut w).context("failed to encode death_location absent")?;
            }
        }
        
        self.portal_cooldown.encode(&mut w).context("failed to encode portal_cooldown")?;
        self.sea_level.encode(&mut w).context("failed to encode sea_level")?;
        
        Ok(())
    }
}

impl SpawnInfo {
    pub fn encoded_len(&self) -> usize {
        let mut buf = Vec::new();
        self.encode(&mut buf).unwrap();
        buf.len()
    }
}

impl<'a> Decode<'a> for SpawnInfo {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let dimension = VarInt::decode(r).context("failed to decode dimension")?;
        let name = String::decode(r).context("failed to decode name")?;
        let hashed_seed = i64::decode(r).context("failed to decode hashed_seed")?;
        let gamemode = u8::decode(r).context("failed to decode gamemode")?;
        let previous_gamemode = i8::decode(r).context("failed to decode previous_gamemode")?;
        let is_debug = bool::decode(r).context("failed to decode is_debug")?;
        let is_flat = bool::decode(r).context("failed to decode is_flat")?;
        
        let has_death_location = bool::decode(r).context("failed to decode has_death_location")?;
        let death_location = if has_death_location {
            Some(GlobalPos::decode(r).context("failed to decode death_location")?)
        } else {
            None
        };
        
        let portal_cooldown = VarInt::decode(r).context("failed to decode portal_cooldown")?;
        let sea_level = VarInt::decode(r).context("failed to decode sea_level")?;

        Ok(Self {
            dimension,
            name,
            hashed_seed,
            gamemode,
            previous_gamemode,
            is_debug,
            is_flat,
            death_location,
            portal_cooldown,
            sea_level,
        })
    }
}