use std::io::Write;
use std::mem;

use anyhow::ensure;
use derive_more::{Deref, DerefMut, From, Into};

use crate::{Bounded, Decode, Encode, VarInt};

/// While [encoding], the contained slice is written directly to the output
/// without any length prefix or metadata.
///
/// While [decoding], the remainder of the input is returned as the contained
/// slice. The input will be at the EOF state after this is decoded.
///
/// [encoding]: Encode
/// [decoding]: Decode
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug, Deref, DerefMut, From, Into,
)]
pub struct RawBytes<'a>(pub &'a [u8]);

impl Encode for RawBytes<'_> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        Ok(w.write_all(self.0)?)
    }
}

impl<'a> Decode<'a> for RawBytes<'a> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(Self(mem::take(r)))
    }
}

/// Special wrapper for Minecraft chunk data that handles the variable-length
/// chunk_data field correctly.
///
/// **Encoding:** Writes raw bytes without any prefix (same as RawBytes).
/// **Decoding:** Reads the chunk data by parsing its internal structure:
///   1. NBT compound (heightmaps) - self-terminating
///   2. VarInt (section data length) + section data bytes
///   3. VarInt (block entities count) + block entity entries
///
/// This allows the decoder to know exactly how many bytes belong to chunk_data
/// without consuming the entire remaining input.
#[derive(Clone, PartialEq, Eq, Debug, Deref, DerefMut)]
pub struct ChunkDataBytes<'a>(pub &'a [u8]);

impl Encode for ChunkDataBytes<'_> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        // Same as RawBytes - no prefix
        Ok(w.write_all(self.0)?)
    }
}

impl<'a> Decode<'a> for ChunkDataBytes<'a> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        eprintln!("🔍 [ChunkDataBytes::decode] CALLED! Input size: {} bytes", r.len());
        eprintln!("🔍 [ChunkDataBytes::decode] First 20 bytes: {:?}", &r[..20.min(r.len())]);
        
        let start = *r;
        let start_len = start.len();
        
        // Parse the internal structure to determine the exact length
        // 1. Skip NBT compound (heightmaps)
        eprintln!("🔍 [ChunkDataBytes::decode] Step 1: Parsing NBT heightmaps...");
        skip_nbt_compound(r)?;
        eprintln!("🔍 [ChunkDataBytes::decode] After heightmaps: {} bytes remaining (consumed {})", r.len(), start_len - r.len());
        
        // 2. Read section data: VarInt(length) + bytes
        eprintln!("🔍 [ChunkDataBytes::decode] Step 2: Reading section data...");
        let section_len = VarInt::decode(r)?.0 as usize;
        eprintln!("🔍 [ChunkDataBytes::decode] Section data length: {} bytes", section_len);
        ensure!(r.len() >= section_len, "not enough bytes for section data: expected {}, got {}", section_len, r.len());
        *r = &r[section_len..];
        eprintln!("🔍 [ChunkDataBytes::decode] After section data: {} bytes remaining", r.len());
        
        // 3. Read block entities: VarInt(count) + entries
        eprintln!("🔍 [ChunkDataBytes::decode] Step 3: Reading block entities...");
        let be_count = VarInt::decode(r)?.0 as usize;
        eprintln!("🔍 [ChunkDataBytes::decode] Block entity count: {}", be_count);
        for i in 0..be_count {
            eprintln!("🔍 [ChunkDataBytes::decode] Reading block entity {}...", i);
            // Each entry: i8(packed_xz) + i16(y) + VarInt(type) + NBT(tag)
            ensure!(r.len() >= 3, "not enough bytes for block entity header");
            *r = &r[3..]; // packed_xz (i8) + y (i16)
            
            let _be_type = VarInt::decode(r)?; // block entity type
            
            // Skip NBT compound
            skip_nbt_compound(r)?;
        }
        
        // Calculate the length consumed
        let end = *r;
        let chunk_data_len = start.len() - end.len();
        let chunk_data = &start[..chunk_data_len];
        
        eprintln!("✅ [ChunkDataBytes::decode] SUCCESS! Consumed {} bytes, {} remaining", chunk_data_len, r.len());
        
        Ok(Self(chunk_data))
    }
}

fn skip_nbt_compound(r: &mut &[u8]) -> anyhow::Result<()> {
    ensure!(!r.is_empty(), "empty input for NBT compound");
    
    let tag_type = r[0];
    *r = &r[1..];
    
    match tag_type {
        // TAG_Compound = 10
        10 => {
            loop {
                ensure!(!r.is_empty(), "unexpected EOF in NBT compound");
                let entry_type = r[0];
                *r = &r[1..];
                
                if entry_type == 0 {
                    return Ok(()); // TAG_End
                }
                
                skip_nbt_string(r)?; // name
                skip_nbt_payload(entry_type, r)?; // value
            }
        }
        _ => Err(anyhow::anyhow!("expected NBT Compound tag (10), got {}", tag_type)),
    }
}

fn skip_nbt_string(r: &mut &[u8]) -> anyhow::Result<()> {
    ensure!(r.len() >= 2, "not enough bytes for string length");
    let len = u16::from_be_bytes([r[0], r[1]]) as usize;
    *r = &r[2..];
    ensure!(r.len() >= len, "not enough bytes for string data");
    *r = &r[len..];
    Ok(())
}

fn skip_nbt_payload(tag_type: u8, r: &mut &[u8]) -> anyhow::Result<()> {
    match tag_type {
        1 => { *r = &r[1..]; } // Byte
        2 => { *r = &r[2..]; } // Short
        3 => { *r = &r[4..]; } // Int
        4 => { *r = &r[8..]; } // Long
        5 => { *r = &r[4..]; } // Float
        6 => { *r = &r[8..]; } // Double
        7 => { // ByteArray
            ensure!(r.len() >= 4, "not enough bytes for byte array length");
            let len = i32::from_be_bytes([r[0], r[1], r[2], r[3]]) as usize;
            *r = &r[4..];
            ensure!(r.len() >= len, "not enough bytes for byte array");
            *r = &r[len..];
        }
        8 => { skip_nbt_string(r)?; } // String
        9 => { // List
            ensure!(r.len() >= 5, "not enough bytes for list header");
            let list_type = r[0];
            let list_len = i32::from_be_bytes([r[1], r[2], r[3], r[4]]) as usize;
            *r = &r[5..];
            for _ in 0..list_len {
                skip_nbt_payload(list_type, r)?;
            }
        }
        10 => { skip_nbt_compound(r)?; } // Compound
        11 => { // LongArray
            ensure!(r.len() >= 4, "not enough bytes for long array length");
            let len = i32::from_be_bytes([r[0], r[1], r[2], r[3]]) as usize;
            *r = &r[4..];
            ensure!(r.len() >= len * 8, "not enough bytes for long array");
            *r = &r[len * 8..];
        }
        _ => return Err(anyhow::anyhow!("unknown NBT tag type: {}", tag_type)),
    }
    Ok(())
}

/// A byte slice that is prefixed with a VarInt length when encoded and decoded.
/// Unlike `RawBytes`, this type only consumes the specified number of bytes during decoding,
/// leaving the remaining input intact for subsequent fields.
///
/// This is useful for packet fields that need to contain variable-length data without
/// consuming the entire remaining input.
#[derive(Clone, PartialEq, Eq, Debug, Deref, DerefMut)]
pub struct BoundedRawBytes<'a>(pub &'a [u8]);

impl Encode for BoundedRawBytes<'_> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.0.len() as i32).encode(&mut w)?;
        w.write_all(self.0)?;
        Ok(())
    }
}

impl<'a> Decode<'a> for BoundedRawBytes<'a> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let len = VarInt::decode(r)?.0 as usize;
        ensure!(r.len() >= len, "not enough bytes: expected {}, got {}", len, r.len());
        let (data, rest) = r.split_at(len);
        *r = rest;
        Ok(Self(data))
    }
}

/// Raises an encoding error if the inner slice is longer than `MAX_BYTES`.
impl<const MAX_BYTES: usize> Encode for Bounded<RawBytes<'_>, MAX_BYTES> {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        ensure!(
            self.len() <= MAX_BYTES,
            "cannot encode more than {MAX_BYTES} raw bytes (got {} bytes)",
            self.len()
        );

        self.0.encode(w)
    }
}

/// Raises a decoding error if the remainder of the input is larger than
/// `MAX_BYTES`.
impl<'a, const MAX_BYTES: usize> Decode<'a> for Bounded<RawBytes<'a>, MAX_BYTES> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        ensure!(
            r.len() <= MAX_BYTES,
            "remainder of input exceeds max of {MAX_BYTES} bytes (got {} bytes)",
            r.len()
        );

        Ok(Bounded(RawBytes::decode(r)?))
    }
}