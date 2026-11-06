// Copyright (c) Microsoft Corporation. All rights reserved.
use std::marker::PhantomData;

use crate::{InvalidEncoding, VerCodable, end_index, read_le_bytes};

mod sealed {
    pub trait Sealed {}
}

/// An unrestricted integer with a fixed-width little-endian wire encoding.
///
/// Implementations cover `u8`, `u16`, `u32`, `u64`, `u128`, `usize`, and their
/// signed counterparts. `usize` and `isize` use the target's pointer width.
/// Portable protocols need explicit-width integers.
///
/// This trait is sealed. Domain validation, options, enums, and framing belong
/// in the enclosing codec, not in a primitive implementation.
pub trait WirePrimitive: sealed::Sealed + Copy {
    /// The number of bytes in this integer's wire encoding.
    const WIDTH: usize;

    /// Reads the first [`Self::WIDTH`] bytes in little-endian order.
    ///
    /// Extra bytes are ignored. Unaligned input is supported.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidEncoding`] if the input is shorter than `Self::WIDTH`.
    fn read_le(buf: &[u8]) -> Result<Self, InvalidEncoding>;

    /// Writes this integer to the first [`Self::WIDTH`] bytes in little-endian order.
    ///
    /// Extra bytes are untouched. Unaligned output is supported.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidEncoding`] without changing the buffer if it is too short.
    fn write_le(self, buf: &mut [u8]) -> Result<(), InvalidEncoding>;
}

/// A zero-sized descriptor for an integer at a fixed byte offset.
///
/// `OFFSET` is relative to the slice supplied to each operation, not necessarily
/// the enclosing frame. The field occupies `T::WIDTH` bytes.
/// Reads and writes affect only that range and support unaligned slices.
///
/// This helper adds no framing, cursor advancement, version selection, or
/// allocation. It does not implement [`crate::VerCodable`].
/// The enclosing codec owns those responsibilities and validates domain values.
///
/// Bound a complete fixed run before accessing its fields. This exposes the
/// run's extent to the optimizer without weakening the checked public interface.
///
/// ```
/// use vercode::{InvalidEncoding, WireField};
///
/// const OFFSET: WireField<u64, 0> = WireField::new();
/// const FILE_ID: WireField<u128, 8> = WireField::new();
///
/// fn example(buf: &mut [u8]) -> Result<u64, InvalidEncoding> {
///     let run = buf.get_mut(..24).ok_or(InvalidEncoding)?;
///     OFFSET.write(run, 1024)?;
///     FILE_ID.write(run, 42)?;
///     OFFSET.read(run)
/// }
///
/// assert_eq!(example(&mut [0; 24]), Ok(1024));
/// assert_eq!(example(&mut [0; 23]), Err(InvalidEncoding));
/// ```
///
/// Restricted values need an integer field and explicit conversion:
///
/// ```compile_fail
/// use vercode::WireField;
/// const FLAG: WireField<bool, 0> = WireField::new();
/// ```
#[derive(Clone, Copy, Debug)]
pub struct WireField<T: WirePrimitive, const OFFSET: usize> {
    marker: PhantomData<T>,
}

impl<T: WirePrimitive, const OFFSET: usize> WireField<T, OFFSET> {
    /// Creates a field descriptor. Operations check the offset against their buffer.
    pub const fn new() -> Self {
        Self {
            marker: PhantomData,
        }
    }

    /// Reads this field without consuming the surrounding bytes.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidEncoding`] if `OFFSET + T::WIDTH` overflows or exceeds
    /// the input length.
    #[inline(always)]
    pub fn read(&self, buf: &[u8]) -> Result<T, InvalidEncoding> {
        let end = end_index(OFFSET, T::WIDTH, buf.len())?;
        T::read_le(buf.get(OFFSET..end).ok_or(InvalidEncoding)?)
    }

    /// Writes this field, leaving all bytes outside its range untouched.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidEncoding`] without changing the buffer if
    /// `OFFSET + T::WIDTH` overflows or exceeds the output length.
    #[inline(always)]
    pub fn write(&self, buf: &mut [u8], value: T) -> Result<(), InvalidEncoding> {
        let end = end_index(OFFSET, T::WIDTH, buf.len())?;
        value.write_le(buf.get_mut(OFFSET..end).ok_or(InvalidEncoding)?)
    }
}

impl<T: WirePrimitive, const OFFSET: usize> Default for WireField<T, OFFSET> {
    fn default() -> Self {
        Self::new()
    }
}

#[inline(always)]
fn write_le_bytes<const N: usize>(buf: &mut [u8], bytes: [u8; N]) -> Result<(), InvalidEncoding> {
    buf.get_mut(..N)
        .ok_or(InvalidEncoding)?
        .copy_from_slice(&bytes);
    Ok(())
}

impl sealed::Sealed for u8 {}
impl WirePrimitive for u8 {
    const WIDTH: usize = size_of::<Self>();

    #[inline(always)]
    fn read_le(buf: &[u8]) -> Result<Self, InvalidEncoding> {
        Ok(Self::from_le_bytes(read_le_bytes(buf)?))
    }

    #[inline(always)]
    fn write_le(self, buf: &mut [u8]) -> Result<(), InvalidEncoding> {
        write_le_bytes(buf, self.to_le_bytes())
    }
}

impl sealed::Sealed for u16 {}
impl WirePrimitive for u16 {
    const WIDTH: usize = size_of::<Self>();

    #[inline(always)]
    fn read_le(buf: &[u8]) -> Result<Self, InvalidEncoding> {
        Ok(Self::from_le_bytes(read_le_bytes(buf)?))
    }

    #[inline(always)]
    fn write_le(self, buf: &mut [u8]) -> Result<(), InvalidEncoding> {
        write_le_bytes(buf, self.to_le_bytes())
    }
}

impl sealed::Sealed for u32 {}
impl WirePrimitive for u32 {
    const WIDTH: usize = size_of::<Self>();

    #[inline(always)]
    fn read_le(buf: &[u8]) -> Result<Self, InvalidEncoding> {
        Ok(Self::from_le_bytes(read_le_bytes(buf)?))
    }

    #[inline(always)]
    fn write_le(self, buf: &mut [u8]) -> Result<(), InvalidEncoding> {
        write_le_bytes(buf, self.to_le_bytes())
    }
}

impl sealed::Sealed for u64 {}
impl WirePrimitive for u64 {
    const WIDTH: usize = size_of::<Self>();

    #[inline(always)]
    fn read_le(buf: &[u8]) -> Result<Self, InvalidEncoding> {
        Ok(Self::from_le_bytes(read_le_bytes(buf)?))
    }

    #[inline(always)]
    fn write_le(self, buf: &mut [u8]) -> Result<(), InvalidEncoding> {
        write_le_bytes(buf, self.to_le_bytes())
    }
}

impl sealed::Sealed for u128 {}
impl WirePrimitive for u128 {
    const WIDTH: usize = size_of::<Self>();

    #[inline(always)]
    fn read_le(buf: &[u8]) -> Result<Self, InvalidEncoding> {
        Ok(Self::from_le_bytes(read_le_bytes(buf)?))
    }

    #[inline(always)]
    fn write_le(self, buf: &mut [u8]) -> Result<(), InvalidEncoding> {
        write_le_bytes(buf, self.to_le_bytes())
    }
}

impl sealed::Sealed for usize {}
impl WirePrimitive for usize {
    const WIDTH: usize = size_of::<Self>();

    #[inline(always)]
    fn read_le(buf: &[u8]) -> Result<Self, InvalidEncoding> {
        Ok(Self::from_le_bytes(read_le_bytes(buf)?))
    }

    #[inline(always)]
    fn write_le(self, buf: &mut [u8]) -> Result<(), InvalidEncoding> {
        write_le_bytes(buf, self.to_le_bytes())
    }
}

impl sealed::Sealed for i8 {}
impl WirePrimitive for i8 {
    const WIDTH: usize = size_of::<Self>();

    #[inline(always)]
    fn read_le(buf: &[u8]) -> Result<Self, InvalidEncoding> {
        Ok(Self::from_le_bytes(read_le_bytes(buf)?))
    }

    #[inline(always)]
    fn write_le(self, buf: &mut [u8]) -> Result<(), InvalidEncoding> {
        write_le_bytes(buf, self.to_le_bytes())
    }
}

impl sealed::Sealed for i16 {}
impl WirePrimitive for i16 {
    const WIDTH: usize = size_of::<Self>();

    #[inline(always)]
    fn read_le(buf: &[u8]) -> Result<Self, InvalidEncoding> {
        Ok(Self::from_le_bytes(read_le_bytes(buf)?))
    }

    #[inline(always)]
    fn write_le(self, buf: &mut [u8]) -> Result<(), InvalidEncoding> {
        write_le_bytes(buf, self.to_le_bytes())
    }
}

impl sealed::Sealed for i32 {}
impl WirePrimitive for i32 {
    const WIDTH: usize = size_of::<Self>();

    #[inline(always)]
    fn read_le(buf: &[u8]) -> Result<Self, InvalidEncoding> {
        Ok(Self::from_le_bytes(read_le_bytes(buf)?))
    }

    #[inline(always)]
    fn write_le(self, buf: &mut [u8]) -> Result<(), InvalidEncoding> {
        write_le_bytes(buf, self.to_le_bytes())
    }
}

impl sealed::Sealed for i64 {}
impl WirePrimitive for i64 {
    const WIDTH: usize = size_of::<Self>();

    #[inline(always)]
    fn read_le(buf: &[u8]) -> Result<Self, InvalidEncoding> {
        Ok(Self::from_le_bytes(read_le_bytes(buf)?))
    }

    #[inline(always)]
    fn write_le(self, buf: &mut [u8]) -> Result<(), InvalidEncoding> {
        write_le_bytes(buf, self.to_le_bytes())
    }
}

impl sealed::Sealed for i128 {}
impl WirePrimitive for i128 {
    const WIDTH: usize = size_of::<Self>();

    #[inline(always)]
    fn read_le(buf: &[u8]) -> Result<Self, InvalidEncoding> {
        Ok(Self::from_le_bytes(read_le_bytes(buf)?))
    }

    #[inline(always)]
    fn write_le(self, buf: &mut [u8]) -> Result<(), InvalidEncoding> {
        write_le_bytes(buf, self.to_le_bytes())
    }
}

impl sealed::Sealed for isize {}
impl WirePrimitive for isize {
    const WIDTH: usize = size_of::<Self>();

    #[inline(always)]
    fn read_le(buf: &[u8]) -> Result<Self, InvalidEncoding> {
        Ok(Self::from_le_bytes(read_le_bytes(buf)?))
    }

    #[inline(always)]
    fn write_le(self, buf: &mut [u8]) -> Result<(), InvalidEncoding> {
        write_le_bytes(buf, self.to_le_bytes())
    }
}

impl VerCodable for u8 {
    const MAX_VERSION: u32 = 0;

    #[inline(always)]
    fn write_version(&self, _version: u32, buf: &mut [u8]) -> Result<usize, InvalidEncoding> {
        self.write_le(buf)?;
        Ok(Self::WIDTH)
    }

    #[inline(always)]
    fn read_version(_version: u32, buf: &[u8]) -> Result<(Self, usize), InvalidEncoding> {
        Ok((Self::read_le(buf)?, Self::WIDTH))
    }

    #[inline(always)]
    fn size_version(&self, _version: u32) -> Result<usize, InvalidEncoding> {
        Ok(Self::WIDTH)
    }
}

impl VerCodable for u16 {
    const MAX_VERSION: u32 = 0;

    #[inline(always)]
    fn write_version(&self, _version: u32, buf: &mut [u8]) -> Result<usize, InvalidEncoding> {
        self.write_le(buf)?;
        Ok(Self::WIDTH)
    }

    #[inline(always)]
    fn read_version(_version: u32, buf: &[u8]) -> Result<(Self, usize), InvalidEncoding> {
        Ok((Self::read_le(buf)?, Self::WIDTH))
    }

    #[inline(always)]
    fn size_version(&self, _version: u32) -> Result<usize, InvalidEncoding> {
        Ok(Self::WIDTH)
    }
}

impl VerCodable for u32 {
    const MAX_VERSION: u32 = 0;

    #[inline(always)]
    fn write_version(&self, _version: u32, buf: &mut [u8]) -> Result<usize, InvalidEncoding> {
        self.write_le(buf)?;
        Ok(Self::WIDTH)
    }

    #[inline(always)]
    fn read_version(_version: u32, buf: &[u8]) -> Result<(Self, usize), InvalidEncoding> {
        Ok((Self::read_le(buf)?, Self::WIDTH))
    }

    #[inline(always)]
    fn size_version(&self, _version: u32) -> Result<usize, InvalidEncoding> {
        Ok(Self::WIDTH)
    }
}

impl VerCodable for u64 {
    const MAX_VERSION: u32 = 0;

    #[inline(always)]
    fn write_version(&self, _version: u32, buf: &mut [u8]) -> Result<usize, InvalidEncoding> {
        self.write_le(buf)?;
        Ok(Self::WIDTH)
    }

    #[inline(always)]
    fn read_version(_version: u32, buf: &[u8]) -> Result<(Self, usize), InvalidEncoding> {
        Ok((Self::read_le(buf)?, Self::WIDTH))
    }

    #[inline(always)]
    fn size_version(&self, _version: u32) -> Result<usize, InvalidEncoding> {
        Ok(Self::WIDTH)
    }
}

impl VerCodable for u128 {
    const MAX_VERSION: u32 = 0;

    #[inline(always)]
    fn write_version(&self, _version: u32, buf: &mut [u8]) -> Result<usize, InvalidEncoding> {
        self.write_le(buf)?;
        Ok(Self::WIDTH)
    }

    #[inline(always)]
    fn read_version(_version: u32, buf: &[u8]) -> Result<(Self, usize), InvalidEncoding> {
        Ok((Self::read_le(buf)?, Self::WIDTH))
    }

    #[inline(always)]
    fn size_version(&self, _version: u32) -> Result<usize, InvalidEncoding> {
        Ok(Self::WIDTH)
    }
}

impl VerCodable for usize {
    const MAX_VERSION: u32 = 0;

    #[inline(always)]
    fn write_version(&self, _version: u32, buf: &mut [u8]) -> Result<usize, InvalidEncoding> {
        self.write_le(buf)?;
        Ok(Self::WIDTH)
    }

    #[inline(always)]
    fn read_version(_version: u32, buf: &[u8]) -> Result<(Self, usize), InvalidEncoding> {
        Ok((Self::read_le(buf)?, Self::WIDTH))
    }

    #[inline(always)]
    fn size_version(&self, _version: u32) -> Result<usize, InvalidEncoding> {
        Ok(Self::WIDTH)
    }
}

impl VerCodable for i8 {
    const MAX_VERSION: u32 = 0;

    #[inline(always)]
    fn write_version(&self, _version: u32, buf: &mut [u8]) -> Result<usize, InvalidEncoding> {
        self.write_le(buf)?;
        Ok(Self::WIDTH)
    }

    #[inline(always)]
    fn read_version(_version: u32, buf: &[u8]) -> Result<(Self, usize), InvalidEncoding> {
        Ok((Self::read_le(buf)?, Self::WIDTH))
    }

    #[inline(always)]
    fn size_version(&self, _version: u32) -> Result<usize, InvalidEncoding> {
        Ok(Self::WIDTH)
    }
}

impl VerCodable for i16 {
    const MAX_VERSION: u32 = 0;

    #[inline(always)]
    fn write_version(&self, _version: u32, buf: &mut [u8]) -> Result<usize, InvalidEncoding> {
        self.write_le(buf)?;
        Ok(Self::WIDTH)
    }

    #[inline(always)]
    fn read_version(_version: u32, buf: &[u8]) -> Result<(Self, usize), InvalidEncoding> {
        Ok((Self::read_le(buf)?, Self::WIDTH))
    }

    #[inline(always)]
    fn size_version(&self, _version: u32) -> Result<usize, InvalidEncoding> {
        Ok(Self::WIDTH)
    }
}

impl VerCodable for i32 {
    const MAX_VERSION: u32 = 0;

    #[inline(always)]
    fn write_version(&self, _version: u32, buf: &mut [u8]) -> Result<usize, InvalidEncoding> {
        self.write_le(buf)?;
        Ok(Self::WIDTH)
    }

    #[inline(always)]
    fn read_version(_version: u32, buf: &[u8]) -> Result<(Self, usize), InvalidEncoding> {
        Ok((Self::read_le(buf)?, Self::WIDTH))
    }

    #[inline(always)]
    fn size_version(&self, _version: u32) -> Result<usize, InvalidEncoding> {
        Ok(Self::WIDTH)
    }
}

impl VerCodable for i64 {
    const MAX_VERSION: u32 = 0;

    #[inline(always)]
    fn write_version(&self, _version: u32, buf: &mut [u8]) -> Result<usize, InvalidEncoding> {
        self.write_le(buf)?;
        Ok(Self::WIDTH)
    }

    #[inline(always)]
    fn read_version(_version: u32, buf: &[u8]) -> Result<(Self, usize), InvalidEncoding> {
        Ok((Self::read_le(buf)?, Self::WIDTH))
    }

    #[inline(always)]
    fn size_version(&self, _version: u32) -> Result<usize, InvalidEncoding> {
        Ok(Self::WIDTH)
    }
}

impl VerCodable for i128 {
    const MAX_VERSION: u32 = 0;

    #[inline(always)]
    fn write_version(&self, _version: u32, buf: &mut [u8]) -> Result<usize, InvalidEncoding> {
        self.write_le(buf)?;
        Ok(Self::WIDTH)
    }

    #[inline(always)]
    fn read_version(_version: u32, buf: &[u8]) -> Result<(Self, usize), InvalidEncoding> {
        Ok((Self::read_le(buf)?, Self::WIDTH))
    }

    #[inline(always)]
    fn size_version(&self, _version: u32) -> Result<usize, InvalidEncoding> {
        Ok(Self::WIDTH)
    }
}

impl VerCodable for isize {
    const MAX_VERSION: u32 = 0;

    #[inline(always)]
    fn write_version(&self, _version: u32, buf: &mut [u8]) -> Result<usize, InvalidEncoding> {
        self.write_le(buf)?;
        Ok(Self::WIDTH)
    }

    #[inline(always)]
    fn read_version(_version: u32, buf: &[u8]) -> Result<(Self, usize), InvalidEncoding> {
        Ok((Self::read_le(buf)?, Self::WIDTH))
    }

    #[inline(always)]
    fn size_version(&self, _version: u32) -> Result<usize, InvalidEncoding> {
        Ok(Self::WIDTH)
    }
}
