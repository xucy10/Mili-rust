use std::borrow::Cow;

use valence_generated::block::BlockEntityKind;
use valence_nbt::Compound;

use crate::{Decode, Encode, Packet};

/// Wrapper type for chunk_data field that provides custom decode logic.
/// This type is only used for decoding; encoding uses the standard RawBytes behavior.
#[derive(Clone, Debug)]
pub struct ChunkDataWrapper<'a>(pub &'a [u8]);

impl<'a> Encode for ChunkDataWrapper<'a> {
    fn encode(&self, mut w: impl std::io::Write) -> anyhow::Result<()> {
        w.write_all(self.0)?;
        Ok(())
    }
}

impl<'a> Decode<'a> for ChunkDataWrapper<'a> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        eprintln!("🔍 [ChunkDataWrapper::decode] CALLED! Input size: {} bytes", r.len());
        
        let start = *r;
        
        // Parse the internal structure to determine exact length
        // 1. Skip NBT compound (heightmaps)
        eprintln!("🔍 [ChunkDataWrapper::decode] Step 1: Parsing NBT heightmaps...");
        skip_nbt_compound(r)?;
        eprintln!("🔍 [ChunkDataWrapper::decode] After heightmaps: {} bytes consumed", start.len() - r.len());
        
        // 2. Read section data: VarInt(length) + bytes
        eprintln!("🔍 [ChunkDataWrapper::decode] Step 2: Reading section data...");
        let section_len = crate::VarInt::decode(r)?.0 as usize;
        eprintln!("🔍 [ChunkDataWrapper::decode] Section data length: {}", section_len);
        anyhow::ensure!(r.len() >= section_len, "not enough bytes for section data");
        *r = &r[section_len..];
        
        // 3. Read block entities: VarInt(count) + entries
        eprintln!("🔍 [ChunkDataWrapper::decode] Step 3: Reading block entities...");
        let be_count = crate::VarInt::decode(r)?.0 as usize;
        eprintln!("🔍 [ChunkDataWrapper::decode] Block entity count: {}", be_count);
        for i in 0..be_count {
            eprintln!("🔍 [ChunkDataWrapper::decode] Reading block entity {}...", i);
            anyhow::ensure!(r.len() >= 3, "not enough bytes for block entity header");
            *r = &r[3..]; // packed_xz (i8) + y (i16)
            
            let _be_type = crate::VarInt::decode(r)?; // block entity type
            
            skip_nbt_compound(r)?; // BlockEntity NBT data
        }
        
        let chunk_data_len = start.len() - r.len();
        eprintln!("✅ [ChunkDataWrapper::decode] SUCCESS! Consumed {} bytes, {} remaining", chunk_data_len, r.len());
        
        Ok(Self(&start[..chunk_data_len]))
    }
}

// Main packet struct - uses derive to get correct Packet ID from macro
#[derive(Clone, Debug, Encode, Decode, Packet)]
pub struct ChunkDataS2c<'a> {
    pub x: i32,
    pub z: i32,
    pub chunk_data: ChunkDataWrapper<'a>,
    pub sky_light_mask: Cow<'a, [u64]>,
    pub block_light_mask: Cow<'a, [u64]>,
    pub empty_sky_light_mask: Cow<'a, [u64]>,
    pub empty_block_light_mask: Cow<'a, [u64]>,
    pub sky_light_arrays: crate::RawBytes<'a>,      // ← 测试改回 RawBytes（无 VarInt 前缀）
    pub block_light_arrays: crate::RawBytes<'a>,     // ← 测试改回 RawBytes（无 VarInt 前缀）
}

#[derive(Clone, PartialEq, Debug, Encode, Decode)]
pub struct ChunkDataBlockEntity<'a> {
    pub packed_xz: i8,
    pub y: i16,
    pub kind: BlockEntityKind,
    pub data: Cow<'a, Compound>,
}

fn skip_nbt_compound(r: &mut &[u8]) -> anyhow::Result<()> {
    anyhow::ensure!(!r.is_empty(), "empty input for NBT compound");
    
    let tag_type = r[0];
    *r = &r[1..];
    
    if tag_type == 10 {
        loop {
            anyhow::ensure!(!r.is_empty(), "unexpected EOF in NBT compound");
            let entry_type = r[0];
            *r = &r[1..];
            
            if entry_type == 0 {
                return Ok(()); // TAG_End
            }
            
            skip_nbt_string(r)?;
            skip_nbt_payload(entry_type, r)?;
        }
    } else {
        Err(anyhow::anyhow!("expected NBT Compound tag (10), got {}", tag_type))
    }
}

fn skip_nbt_string(r: &mut &[u8]) -> anyhow::Result<()> {
    anyhow::ensure!(r.len() >= 2, "not enough bytes for string length");
    let len = u16::from_be_bytes([r[0], r[1]]) as usize;
    *r = &r[2..];
    anyhow::ensure!(r.len() >= len, "not enough bytes for string data");
    *r = &r[len..];
    Ok(())
}

fn skip_nbt_payload(tag_type: u8, r: &mut &[u8]) -> anyhow::Result<()> {
    match tag_type {
        1 => { *r = &r[1..]; }
        2 => { *r = &r[2..]; }
        3 => { *r = &r[4..]; }
        4 => { *r = &r[8..]; }
        5 => { *r = &r[4..]; }
        6 => { *r = &r[8..]; }
        7 => {
            anyhow::ensure!(r.len() >= 4, "not enough bytes for byte array length");
            let len = i32::from_be_bytes([r[0], r[1], r[2], r[3]]) as usize;
            *r = &r[4..];
            anyhow::ensure!(r.len() >= len, "not enough bytes for byte array");
            *r = &r[len..];
        }
        8 => { skip_nbt_string(r)?; }
        9 => {
            anyhow::ensure!(r.len() >= 5, "not enough bytes for list header");
            let list_type = r[0];
            let list_len = i32::from_be_bytes([r[1], r[2], r[3], r[4]]) as usize;
            *r = &r[5..];
            for _ in 0..list_len {
                skip_nbt_payload(list_type, r)?;
            }
        }
        10 => { skip_nbt_compound(r)?; }
        11 => {
            anyhow::ensure!(r.len() >= 4, "not enough bytes for long array length");
            let len = i32::from_be_bytes([r[0], r[1], r[2], r[3]]) as usize;
            *r = &r[4..];
            anyhow::ensure!(r.len() >= len * 8, "not enough bytes for long array");
            *r = &r[len * 8..];
        }
        _ => return Err(anyhow::anyhow!("unknown NBT tag type: {}", tag_type)),
    }
    Ok(())
}