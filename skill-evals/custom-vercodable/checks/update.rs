use bincode::Options;
use custom_vercodable_eval::{Child, Packet};
use serde::{Deserialize, Serialize};

#[path = "../historical.rs"]
#[allow(clippy::chunks_exact_to_as_chunks)]
mod historical;

include!("skill_eval_support/common.rs");

type Custom = Packet;

fn frame_layout(bytes: &[u8]) -> (usize, usize) {
    let count = u64::from_le_bytes(bytes[8..16].try_into().unwrap()) as usize;
    let base = 16 + 2 * count;
    (base, base + 4)
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct BincodeChild {
    low: u16,
    high: u16,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct BincodePacket {
    child: BincodeChild,
    marker: u32,
    values: Vec<u16>,
}

fn packet() -> Packet {
    Packet {
        child: Child {
            low: 0x1234,
            high: 0xabcd,
            stamp: 0x0123_4567_89ab_cdef,
        },
        marker: 0x7654_3210,
        values: vec![0, 1, 0x789a, u16::MAX],
        enabled: true,
        note: "updated-\u{03bb}".into(),
    }
}

fn old() -> historical::Packet {
    historical::Packet {
        child: historical::Child {
            low: 0x1234,
            high: 0xabcd,
        },
        marker: 0x7654_3210,
        values: vec![0, 1, 0x789a, u16::MAX],
        enabled: true,
    }
}

fn bincode_value(value: &Packet) -> BincodePacket {
    BincodePacket {
        child: BincodeChild {
            low: value.child.low,
            high: value.child.high,
        },
        marker: value.marker,
        values: value.values.clone(),
    }
}

fn projected(value: &Packet, version: u32) -> Packet {
    let mut value = value.clone();
    if version < 1 {
        value.enabled = false;
    }
    if version < 2 {
        value.child.stamp = 0;
    }
    if version < 3 {
        value.note.clear();
    }
    value
}

fn wire(value: &Packet, version: u32) -> Vec<u8> {
    let mut bytes = bincode::serialize(&bincode_value(value)).unwrap();
    bytes.extend([0; 4]);
    if version >= 1 {
        bytes.push(u8::from(value.enabled));
    }
    if version >= 2 {
        bytes.extend(value.child.stamp.to_le_bytes());
    }
    if version >= 3 {
        bytes.extend((value.note.len() as u64).to_le_bytes());
        bytes.extend(value.note.as_bytes());
    }
    reframe(&mut bytes);
    bytes
}

#[test]
fn literal_bincode_base_has_fixed_int_u64_count() {
    let expected = [
        0x34, 0x12, 0xcd, 0xab, 0x10, 0x32, 0x54, 0x76, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0x9a,
        0x78, 0xff, 0xff,
    ];
    assert_eq!(
        bincode::serialize(&bincode_value(&packet())).unwrap(),
        expected
    );
    let mut adopted = expected.to_vec();
    adopted.extend([0; 4]);
    assert_eq!(encode(&packet(), 0), adopted);
    assert_eq!(encode(&old(), 0), adopted);
    assert_eq!(packet().size_version(0), Ok(expected.len() + 4));
}

#[test]
fn post_bootstrap_v0_v1_bytes_are_unchanged() {
    for version in [0, 1] {
        assert_eq!(encode(&packet(), version), encode(&old(), version));
        assert_eq!(encode(&packet(), version), wire(&packet(), version));
    }
}

#[test]
fn bare_pre_bootstrap_bincode_requires_explicit_migration() {
    let value = packet();
    let bytes = bincode::serialize(&bincode_value(&value)).unwrap();
    for version in 0..=3 {
        assert_eq!(Packet::read_version(version, &bytes), Err(InvalidEncoding));
    }
    // Explicit migration: keep the actual bincode base, append the mandatory header.
    let mut migrated = bytes.clone();
    migrated.extend([0; 4]);
    assert_eq!(
        Packet::read_version(3, &migrated),
        Ok((projected(&value, 0), migrated.len()))
    );
    // A nonempty following sibling is not evidence that the missing header is optional.
    let mut legacy_stream = bytes;
    legacy_stream.extend(u32::MAX.to_le_bytes());
    assert_eq!(
        Packet::read_version(0, &legacy_stream),
        Err(InvalidEncoding)
    );
}

#[test]
fn frozen_old_writers_default_child_and_root_additions() {
    for version in [0, 1] {
        let bytes = encode(&old(), version);
        assert_eq!(
            Packet::read_version(3, &bytes),
            Ok((projected(&packet(), version), bytes.len()))
        );
        check_following_sibling(&bytes, projected(&packet(), version), 3);
    }
}

#[test]
fn post_bootstrap_old_readers_consume_full_frame_not_remainder() {
    for writer in 0..=3 {
        let bytes = encode(&packet(), writer);
        let mut stream = bytes.clone();
        stream.extend([0xa5; 7]);
        for reader in [0, 1] {
            let mut expected = old();
            expected.enabled = writer.min(reader) >= 1;
            assert_eq!(
                historical::Packet::read_version(reader, &stream),
                Ok((expected, bytes.len()))
            );
        }
    }
}

#[test]
fn bincode_reader_has_explicit_trailing_byte_policy() {
    let base = bincode::serialize(&bincode_value(&packet())).unwrap();
    let latest = encode(&packet(), 3);
    let expected = bincode_value(&packet());
    let exact = bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_little_endian()
        .reject_trailing_bytes();
    assert_eq!(exact.deserialize::<BincodePacket>(&base).unwrap(), expected);
    assert!(
        exact
            .deserialize::<BincodePacket>(&wire(&packet(), 0))
            .is_err()
    );
    assert!(exact.deserialize::<BincodePacket>(&latest).is_err());
    let forward = bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_little_endian()
        .allow_trailing_bytes();
    assert_eq!(
        forward.deserialize::<BincodePacket>(&latest).unwrap(),
        expected
    );
    // DefaultOptions uses variable integers, unlike the top-level serialize API.
    assert_ne!(
        bincode::DefaultOptions::new().serialize(&expected).unwrap(),
        base
    );
}

#[test]
fn unchanged_bincode_stream_reader_does_not_consume_extension_metadata() {
    let bytes = encode(&packet(), 3);
    let mut cursor = std::io::Cursor::new(&bytes);
    let value: BincodePacket = bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_little_endian()
        .allow_trailing_bytes()
        .deserialize_from(&mut cursor)
        .unwrap();
    assert_eq!(value, bincode_value(&packet()));
    let base = bincode::serialize(&bincode_value(&packet())).unwrap().len();
    assert_eq!(cursor.position() as usize, base);
    assert_eq!(
        u32::from_le_bytes(bytes[base..base + 4].try_into().unwrap()) as usize,
        bytes.len() - base - 4
    );
    assert_eq!(bytes[base + 4], 1);
}

#[test]
fn child_addition_follows_enabled_not_child_base() {
    let value = packet();
    let base = wire(&value, 0);
    let one = encode(&value, 1);
    let two = encode(&value, 2);
    let three = encode(&value, 3);
    let header = base.len() - 4;
    assert_eq!(&one[..header], &base[..header]);
    assert_eq!(&one[base.len()..], &[1]);
    assert_eq!(&two[..header], &base[..header]);
    assert_eq!(&two[base.len()..one.len()], &one[base.len()..]);
    assert_eq!(&two[one.len()..], &value.child.stamp.to_le_bytes());
    assert_eq!(&three[..header], &base[..header]);
    assert_eq!(&three[base.len()..two.len()], &two[base.len()..]);
    assert_eq!(
        &three[two.len()..two.len() + 8],
        &(value.note.len() as u64).to_le_bytes()
    );
}

#[test]
fn all_writer_reader_version_pairs_match_independent_oracle() {
    assert_eq!(Packet::MAX_VERSION, 3);
    let value = packet();
    for writer in [0, 1, 2, 3, 7, u32::MAX] {
        let bytes = encode(&value, writer);
        assert_eq!(bytes, wire(&value, writer));
        let (header, extensions) = frame_layout(&bytes);
        assert_eq!(value.size_version(writer), Ok(bytes.len()));
        assert_eq!(
            u32::from_le_bytes(bytes[header..extensions].try_into().unwrap()) as usize,
            bytes.len() - extensions
        );
        for reader in [0, 1, 2, 3, 7, u32::MAX] {
            assert_eq!(
                Packet::read_version(reader, &bytes),
                Ok((projected(&value, writer.min(reader)), bytes.len()))
            );
        }
    }
}

#[test]
fn truncated_frames_fail_and_reframed_generations_default() {
    for value in [packet(), Packet::default()] {
        for version in 0..=3 {
            let boundaries: Vec<_> = (0..version).map(|v| wire(&value, v).len()).collect();
            check_buffers(&value, version, &boundaries);
        }
    }
}

#[test]
fn partial_stamp_batch_never_defaults() {
    let one = wire(&packet(), 1);
    for length in 1..8 {
        let mut bad = one.clone();
        bad.extend(&packet().child.stamp.to_le_bytes()[..length]);
        reframe(&mut bad);
        assert_eq!(Packet::read_version(3, &bad), Err(InvalidEncoding));
    }
}

#[test]
fn partial_note_length_and_payload_never_default() {
    let two = wire(&packet(), 2);
    for extra in [
        &[5, 0][..],
        &[5, 0, 0, 0, 0, 0, 0, 0][..],
        &[5, 0, 0, 0, 0, 0, 0, 0, b'a'][..],
    ] {
        let mut bad = two.clone();
        bad.extend(extra);
        reframe(&mut bad);
        assert_eq!(Packet::read_version(3, &bad), Err(InvalidEncoding));
    }
}

#[test]
fn invalid_bool_utf8_and_u64_lengths_fail() {
    for (offset, replacement) in [
        (wire(&packet(), 0).len(), vec![2]),
        (8, u64::MAX.to_le_bytes().to_vec()),
        (wire(&packet(), 2).len(), u64::MAX.to_le_bytes().to_vec()),
        (wire(&packet(), 2).len() + 8, vec![0xff]),
    ] {
        let mut bad = wire(&packet(), 3);
        bad[offset..offset + replacement.len()].copy_from_slice(&replacement);
        assert_eq!(Packet::read_version(3, &bad), Err(InvalidEncoding));
    }
}

#[test]
fn future_suffix_uses_all_extensions_length_and_preserves_sibling() {
    let mut bytes = wire(&packet(), 3);
    bytes.extend([0xff, 0, 0x80, 9]);
    reframe(&mut bytes);
    for version in 0..=3 {
        check_following_sibling(&bytes, projected(&packet(), version), version);
    }
    let mut stream = bytes.clone();
    stream.extend(0x1234_5678u32.to_le_bytes());
    for version in [0, 1] {
        let mut expected = old();
        expected.enabled = version >= 1;
        assert_eq!(
            historical::Packet::read_version(version, &stream),
            Ok((expected, bytes.len()))
        );
    }
}

#[test]
fn generic_vec_and_tuple_preserve_following_values() {
    generic_composition(&[packet(), Packet::default(), packet()], 3);
}

#[test]
fn macro_parent_preserves_trailing_and_versioned_fields() {
    derived_parent_composition(packet());
}

#[test]
fn mandatory_extension_header_missing_partial_or_overflowing_fails() {
    let bytes = wire(&packet(), 3);
    invalid_frame_lengths::<Packet>(&bytes);
    let base = bincode::serialize(&bincode_value(&packet())).unwrap();
    for missing in 0..4 {
        let mut truncated = base.clone();
        truncated.extend(&[0; 4][..missing]);
        for version in [0, 1, 3] {
            assert_eq!(
                Packet::read_version(version, &truncated),
                Err(InvalidEncoding)
            );
            assert_eq!(
                historical::Packet::read_version(version, &truncated),
                Err(InvalidEncoding)
            );
        }
    }
}

#[test]
fn zero_extension_header_does_not_consume_sibling_as_enabled() {
    let bytes = wire(&packet(), 0);
    check_following_sibling(&bytes, projected(&packet(), 0), 3);
    let mut stream = bytes.clone();
    stream.extend([1, 0, 0, 0]);
    let mut expected = old();
    expected.enabled = false;
    assert_eq!(
        historical::Packet::read_version(1, &stream),
        Ok((expected, bytes.len()))
    );
}

#[test]
fn empty_values_and_unaligned_input() {
    let value = Packet::default();
    let bytes = wire(&value, 3);
    let mut unaligned = vec![0xff];
    unaligned.extend(&bytes);
    assert_eq!(
        Packet::read_version(3, &unaligned[1..]),
        Ok((value, bytes.len()))
    );
}

#[test]
fn malformed_input_smoke_does_not_panic() {
    malformed_smoke::<Packet>();
}
