// Copyright (c) Microsoft Corporation. All rights reserved.
use std::fmt::Debug;
use std::num::NonZeroU64;

use vercode::{InvalidEncoding, VerCodable, Vercode, WireField, WirePrimitive};

fn primitive<T: WirePrimitive + VerCodable + Debug + PartialEq>(value: T, expected: &[u8]) {
    let field = WireField::<T, 2>::new();
    let mut output = [0xa5; 40];
    field.write(&mut output[1..], value).unwrap();
    assert_eq!(&output[..3], &[0xa5; 3]);
    assert_eq!(&output[3..3 + expected.len()], expected);
    assert!(
        output[3 + expected.len()..]
            .iter()
            .all(|byte| *byte == 0xa5)
    );
    assert_eq!(field.read(&output[1..]), Ok(value));
    assert_eq!(T::read_le(&output[3..]), Ok(value));

    let mut original = [0xa5; 40];
    assert_eq!(
        value.write_version(u32::MAX, &mut original).unwrap(),
        expected.len()
    );
    assert_eq!(&original[..expected.len()], expected);
    assert_eq!(T::read_le(&original), Ok(value));
    assert_eq!(
        T::read_version(0, &output[3..]),
        Ok((value, expected.len()))
    );

    let mut direct = [0xa5; 40];
    value.write_le(&mut direct[1..]).unwrap();
    assert_eq!(direct[0], 0xa5);
    assert_eq!(&direct[1..1 + expected.len()], expected);
    assert!(
        direct[1 + expected.len()..]
            .iter()
            .all(|byte| *byte == 0xa5)
    );

    for length in 0..2 + expected.len() {
        assert_eq!(field.read(&output[1..1 + length]), Err(InvalidEncoding));
        let before = output;
        assert_eq!(
            field.write(&mut output[1..1 + length], value),
            Err(InvalidEncoding)
        );
        assert_eq!(output, before);
    }
    for length in 0..expected.len() {
        assert_eq!(T::read_le(&direct[1..1 + length]), Err(InvalidEncoding));
        let before = direct;
        assert_eq!(
            value.write_le(&mut direct[1..1 + length]),
            Err(InvalidEncoding)
        );
        assert_eq!(direct, before);
    }
}

#[test]
fn unsigned_integer_bytes_and_bounds() {
    primitive(0u8, &[0]);
    primitive(u8::MAX, &[0xff]);
    primitive(0xabu8, &[0xab]);
    primitive(0u16, &[0; 2]);
    primitive(u16::MAX, &[0xff; 2]);
    primitive(0x89abu16, &[0xab, 0x89]);
    primitive(0u32, &[0; 4]);
    primitive(u32::MAX, &[0xff; 4]);
    primitive(0x89ab_cdefu32, &[0xef, 0xcd, 0xab, 0x89]);
    primitive(0u64, &[0; 8]);
    primitive(u64::MAX, &[0xff; 8]);
    primitive(
        0x89ab_cdef_0123_4567u64,
        &[0x67, 0x45, 0x23, 0x01, 0xef, 0xcd, 0xab, 0x89],
    );
    primitive(0u128, &[0; 16]);
    primitive(u128::MAX, &[0xff; 16]);
    primitive(
        0xfedc_ba98_7654_3210_0123_4567_89ab_cdefu128,
        &[
            0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01, 0x10, 0x32, 0x54, 0x76, 0x98, 0xba,
            0xdc, 0xfe,
        ],
    );
}

#[test]
fn signed_integer_bytes_and_bounds() {
    primitive(i8::MIN, &[0x80]);
    primitive(i8::MAX, &[0x7f]);
    primitive(-1i8, &[0xff]);
    primitive(i16::MIN, &[0, 0x80]);
    primitive(i16::MAX, &[0xff, 0x7f]);
    primitive(-0x1234i16, &[0xcc, 0xed]);
    primitive(i32::MIN, &[0, 0, 0, 0x80]);
    primitive(i32::MAX, &[0xff, 0xff, 0xff, 0x7f]);
    primitive(-1i32, &[0xff; 4]);
    primitive(i64::MIN, &[0, 0, 0, 0, 0, 0, 0, 0x80]);
    primitive(i64::MAX, &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f]);
    primitive(-1i64, &[0xff; 8]);
    primitive(
        i128::MIN,
        &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x80],
    );
    primitive(
        i128::MAX,
        &[
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0x7f,
        ],
    );
    primitive(-1i128, &[0xff; 16]);
}

#[test]
fn pointer_sized_integers_retain_target_width() {
    let width = size_of::<usize>();
    let mut high_bit = vec![0; width];
    high_bit[width - 1] = 0x80;
    let mut signed_max = vec![0xff; width];
    signed_max[width - 1] = 0x7f;
    primitive(0usize, &vec![0; width]);
    primitive(usize::MAX, &vec![0xff; width]);
    primitive(1usize << (usize::BITS - 1), &high_bit);
    primitive(isize::MIN, &high_bit);
    primitive(isize::MAX, &signed_max);
    primitive(-1isize, &vec![0xff; width]);
}

fn rejected_offset<const OFFSET: usize>() {
    let field = WireField::<u64, OFFSET>::new();
    let mut output = [0xa5; 32];
    assert_eq!(field.read(&output), Err(InvalidEncoding));
    assert_eq!(field.write(&mut output, 123), Err(InvalidEncoding));
    assert_eq!(output, [0xa5; 32]);
}

#[test]
fn invalid_offsets_fail_without_mutation_or_panics() {
    rejected_offset::<40>();
    rejected_offset::<{ usize::MAX }>();
    rejected_offset::<{ usize::MAX - 3 }>();
    rejected_offset::<{ usize::MAX - 8 }>();
}

#[derive(Debug, PartialEq, Vercode)]
struct HistoricalPacket {
    id: u32,
    ready: bool,
    sequence: Option<NonZeroU64>,
}

#[derive(Debug, PartialEq)]
struct Packet {
    id: u32,
    ready: bool,
    sequence: Option<NonZeroU64>,
    note: String,
}

mod base {
    use vercode::WireField;

    pub const SIZE: usize = 17;
    pub const TOTAL: WireField<u32, 0> = WireField::new();
    pub const ID: WireField<u32, 4> = WireField::new();
    pub const READY: WireField<u8, 8> = WireField::new();
    pub const SEQUENCE: WireField<u64, 9> = WireField::new();
}

impl VerCodable for Packet {
    const MAX_VERSION: u32 = 1;

    fn write_version(&self, version: u32, buf: &mut [u8]) -> Result<usize, InvalidEncoding> {
        let run = buf.get_mut(..base::SIZE).ok_or(InvalidEncoding)?;
        base::ID.write(run, self.id)?;
        base::READY.write(run, u8::from(self.ready))?;
        base::SEQUENCE.write(run, self.sequence.map_or(0, NonZeroU64::get))?;
        let mut total = base::SIZE;
        if version >= 1 {
            total = total
                .checked_add(self.note.write_version(version, &mut buf[base::SIZE..])?)
                .ok_or(InvalidEncoding)?;
        }
        base::TOTAL.write(buf, u32::try_from(total).map_err(|_| InvalidEncoding)?)?;
        Ok(total)
    }

    fn read_version(version: u32, buf: &[u8]) -> Result<(Self, usize), InvalidEncoding> {
        let total = base::TOTAL.read(buf)? as usize;
        let frame = buf.get(..total).ok_or(InvalidEncoding)?;
        let run = frame.get(..base::SIZE).ok_or(InvalidEncoding)?;
        let ready = match base::READY.read(run)? {
            0 => false,
            1 => true,
            _ => return Err(InvalidEncoding),
        };
        let mut result = Self {
            id: base::ID.read(run)?,
            ready,
            sequence: NonZeroU64::new(base::SEQUENCE.read(run)?),
            note: String::new(),
        };
        if version >= 1 && frame.len() > base::SIZE {
            result.note = String::read_version(version, &frame[base::SIZE..])?.0;
        }
        Ok((result, total))
    }

    fn size_version(&self, version: u32) -> Result<usize, InvalidEncoding> {
        let extension = if version >= 1 {
            self.note.size_version(version)?
        } else {
            0
        };
        let total = base::SIZE.checked_add(extension).ok_or(InvalidEncoding)?;
        u32::try_from(total).map_err(|_| InvalidEncoding)?;
        Ok(total)
    }
}

fn packet() -> Packet {
    Packet {
        id: 0x1122_3344,
        ready: true,
        sequence: NonZeroU64::new(0x0102_0304_0506_0708),
        note: "hi".into(),
    }
}

#[test]
fn shared_fields_preserve_independent_history_and_future_consumption() {
    let value = packet();
    let historical = HistoricalPacket {
        id: value.id,
        ready: value.ready,
        sequence: value.sequence,
    };
    let golden = [
        17, 0, 0, 0, 0x44, 0x33, 0x22, 0x11, 1, 8, 7, 6, 5, 4, 3, 2, 1,
    ];
    assert_eq!(vercode::serialize_to_vec(&historical).unwrap(), golden);
    let mut storage = [0xa5; 64];
    assert_eq!(
        value.write_version(0, &mut storage[1..]).unwrap(),
        golden.len()
    );
    assert_eq!(&storage[1..18], &golden);
    let mut old_projection = packet();
    old_projection.note.clear();
    assert_eq!(
        Packet::read_version(1, &golden),
        Ok((old_projection, golden.len()))
    );

    let mut future = vercode::serialize_to_vec(&value).unwrap();
    assert_eq!(&future[4..17], &golden[4..]);
    assert_eq!(&future[17..], &[2, 0, 0, 0, b'h', b'i']);
    future.extend_from_slice(&[0xde, 0xad]);
    let total = future.len();
    future[..4].copy_from_slice(&(total as u32).to_le_bytes());
    future.extend_from_slice(&0x8765_4321u32.to_le_bytes());
    assert_eq!(Packet::read_version(1, &future), Ok((packet(), total)));
    assert_eq!(
        <(HistoricalPacket, u32)>::read_version(0, &future),
        Ok(((historical, 0x8765_4321), future.len()))
    );
    assert_eq!(Packet::read_version(0, &future).unwrap().1, total);
}

#[test]
fn enclosing_codec_preserves_errors_sizes_and_domain_conversions() {
    for sequence in [None, NonZeroU64::new(123)] {
        let mut value = packet();
        value.sequence = sequence;
        for version in [0, 1, u32::MAX] {
            let size = value.size_version(version).unwrap();
            let mut output = vec![0xa5; size + 2];
            assert_eq!(value.write_version(version, &mut output[1..]), Ok(size));
            assert_eq!(output[0], 0xa5);
            assert_eq!(output[size + 1], 0xa5);
            let bytes = &output[1..size + 1];
            let decoded = Packet::read_version(version, bytes).unwrap().0;
            assert_eq!(decoded.sequence, sequence);
            assert_eq!(decoded.note, if version == 0 { "" } else { "hi" });
            for length in 0..size {
                assert_eq!(
                    Packet::read_version(version, &bytes[..length]),
                    Err(InvalidEncoding)
                );
                assert_eq!(
                    value.write_version(version, &mut vec![0; length]),
                    Err(InvalidEncoding)
                );
            }
            let mut invalid = bytes.to_vec();
            invalid[8] = 2;
            assert_eq!(
                Packet::read_version(version, &invalid),
                Err(InvalidEncoding)
            );
        }
    }
}
