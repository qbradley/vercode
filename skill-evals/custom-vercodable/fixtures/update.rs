use hashed_type_def::HashedTypeDef;
use vercode::{InvalidEncoding, VerCodable};

#[derive(Debug, Default, PartialEq, Clone, HashedTypeDef)]
pub struct Child {
    pub low: u16,
    pub high: u16,
}

#[derive(Debug, Default, PartialEq, Clone, HashedTypeDef)]
pub struct Packet {
    pub child: Child,
    pub marker: u32,
    pub values: Vec<u16>,
    /// Global pass 1.
    pub enabled: bool,
}

// Adopted bincode 1.3.3 serialize format: fixed integers, little endian,
// low:u16, high:u16, marker:u32, count:u64, values:[u16; count].
// POST-BOOTSTRAP historical adapter, not an unchanged pre-bootstrap bincode reader.
// A mandatory u32 LE extension BYTE length follows the COMPLETE bincode base.
// Even v0 writes the zero-length header. Pass 1 then appends enabled:u8.
// The header covers ALL extension passes, including unknown future bytes.
// Return base length + 4 + declared extension length, preserving following values.
// Bare pre-bootstrap bincode requires explicit migration or a separate legacy API.
// There are no nested frames, version words, or per-pass lengths.
//
// Ask an LLM to use custom-vercodable in update mode after a schema change.
// Preserve the base bytes and earlier passes, update write/read/size together,
// test against historical.rs AND real bincode, then refresh the literal guard.
// Do not refresh the hash alone. New graph-wide introductions exceed version 1.
const _: () = assert!(
    <Packet as HashedTypeDef>::TYPE_HASH_NATIVE == 0x12010dcfbbb1e5a3ee3b4ca8c01bdb23,
    "Packet type tree changed. Ask an LLM to update the implementation with \
     custom-vercodable in update mode, preserve historical compatibility, \
     run historical tests, then fix this assert."
);

// HashedTypeDef's Vec boundary does not prove its actual argument. This witness
// is tied to the real field, not a copied proxy definition.
const fn actual_field_hash<T: HashedTypeDef>(_: fn(&Packet) -> &T) -> u128 {
    T::TYPE_HASH_NATIVE
}

const _: () = assert!(
    actual_field_hash(|packet| &packet.values[0]) == <u16 as HashedTypeDef>::TYPE_HASH_NATIVE,
    "Packet values element type tree changed. Ask an LLM to use custom-vercodable \
     in update mode, preserve history, update all methods, and rerun tests."
);

impl VerCodable for Packet {
    const MAX_VERSION: u32 = 1;

    fn write_version(&self, version: u32, buf: &mut [u8]) -> Result<usize, InvalidEncoding> {
        let length = self.size_version(version)?;
        let output = buf.get_mut(..length).ok_or(InvalidEncoding)?;
        output[..2].copy_from_slice(&self.child.low.to_le_bytes());
        output[2..4].copy_from_slice(&self.child.high.to_le_bytes());
        output[4..8].copy_from_slice(&self.marker.to_le_bytes());
        let count = u64::try_from(self.values.len()).map_err(|_| InvalidEncoding)?;
        output[8..16].copy_from_slice(&count.to_le_bytes());
        for (value, slot) in self.values.iter().zip(output[16..].chunks_exact_mut(2)) {
            slot.copy_from_slice(&value.to_le_bytes());
        }
        let base = 16 + self.values.len() * 2;
        output[base..base + 4].copy_from_slice(&u32::from(version >= 1).to_le_bytes());
        if version >= 1 {
            output[length - 1] = u8::from(self.enabled);
        }
        Ok(length)
    }

    fn read_version(version: u32, buf: &[u8]) -> Result<(Self, usize), InvalidEncoding> {
        let header = buf.get(..16).ok_or(InvalidEncoding)?;
        let low = u16::from_le_bytes(header[..2].try_into().map_err(|_| InvalidEncoding)?);
        let high = u16::from_le_bytes(header[2..4].try_into().map_err(|_| InvalidEncoding)?);
        let marker = u32::from_le_bytes(header[4..8].try_into().map_err(|_| InvalidEncoding)?);
        let count = u64::from_le_bytes(header[8..16].try_into().map_err(|_| InvalidEncoding)?);
        let count = usize::try_from(count).map_err(|_| InvalidEncoding)?;
        let end = count
            .checked_mul(2)
            .and_then(|length| length.checked_add(16))
            .ok_or(InvalidEncoding)?;
        let payload = buf.get(16..end).ok_or(InvalidEncoding)?;
        let values = payload
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        let extensions = end.checked_add(4).ok_or(InvalidEncoding)?;
        let length = u32::from_le_bytes(
            buf.get(end..extensions)
                .ok_or(InvalidEncoding)?
                .try_into()
                .map_err(|_| InvalidEncoding)?,
        );
        let total = extensions
            .checked_add(usize::try_from(length).map_err(|_| InvalidEncoding)?)
            .ok_or(InvalidEncoding)?;
        let extension = buf.get(extensions..total).ok_or(InvalidEncoding)?;
        let enabled = if version >= 1 && !extension.is_empty() {
            match extension[0] {
                0 => false,
                1 => true,
                _ => return Err(InvalidEncoding),
            }
        } else {
            false
        };
        Ok((
            Self {
                child: Child { low, high },
                marker,
                values,
                enabled,
            },
            total,
        ))
    }

    fn size_version(&self, version: u32) -> Result<usize, InvalidEncoding> {
        u64::try_from(self.values.len()).map_err(|_| InvalidEncoding)?;
        self.values
            .len()
            .checked_mul(2)
            .and_then(|length| length.checked_add(20 + usize::from(version >= 1)))
            .ok_or(InvalidEncoding)
    }
}
