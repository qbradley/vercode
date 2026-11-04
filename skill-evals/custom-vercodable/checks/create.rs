use custom_vercodable_eval::{Entry, Point, Sample};
use std::num::NonZeroU32;
use vercode::Vercode;

include!("skill_eval_support/common.rs");

type Custom = Sample;

fn frame_layout(_: &[u8]) -> (usize, usize) {
    (0, 0)
}

#[derive(Debug, PartialEq, Vercode)]
struct RefPoint {
    x: u16,
    y: u16,
    #[version(1)]
    confidence: u8,
}

#[derive(Debug, PartialEq, Vercode)]
struct RefEntry {
    point: RefPoint,
    weight: u32,
}

#[derive(Debug, PartialEq, Vercode)]
struct RefSample {
    id: u64,
    point: RefPoint,
    tail: u32,
    flags: Option<bool>,
    ticket: Option<NonZeroU32>,
    bytes: [u8; 8],
    samples: Vec<u16>,
    entries: Vec<RefEntry>,
    #[version(2)]
    label: String,
}

fn sample() -> Sample {
    Sample {
        id: 1,
        point: Point {
            x: 2,
            y: 3,
            confidence: 90,
        },
        tail: 4,
        flags: Some(true),
        ticket: NonZeroU32::new(5),
        bytes: [0, 1, 2, 3, 4, 5, 6, 7],
        samples: vec![0x1122, 0x3344],
        entries: vec![
            Entry {
                point: Point {
                    x: 6,
                    y: 7,
                    confidence: 91,
                },
                weight: 8,
            },
            Entry {
                point: Point {
                    x: 9,
                    y: 10,
                    confidence: 92,
                },
                weight: 11,
            },
        ],
        label: "wire-\u{03bb}".into(),
    }
}

fn ref_point(point: &Point) -> RefPoint {
    RefPoint {
        x: point.x,
        y: point.y,
        confidence: point.confidence,
    }
}

fn reference(value: &Sample) -> RefSample {
    RefSample {
        id: value.id,
        point: ref_point(&value.point),
        tail: value.tail,
        flags: value.flags,
        ticket: value.ticket,
        bytes: value.bytes,
        samples: value.samples.clone(),
        entries: value
            .entries
            .iter()
            .map(|entry| RefEntry {
                point: ref_point(&entry.point),
                weight: entry.weight,
            })
            .collect(),
        label: value.label.clone(),
    }
}

fn projected(value: &Sample, version: u32) -> Sample {
    let mut value = value.clone();
    if version < 1 {
        value.point.confidence = 0;
        for entry in &mut value.entries {
            entry.point.confidence = 0;
        }
    }
    if version < 2 {
        value.label.clear();
    }
    value
}

fn wire(value: &Sample, version: u32) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(value.id.to_le_bytes());
    out.extend(value.point.x.to_le_bytes());
    out.extend(value.point.y.to_le_bytes());
    out.extend(value.tail.to_le_bytes());
    out.push(match value.flags {
        Some(false) => 0,
        Some(true) => 1,
        None => 2,
    });
    out.extend(value.ticket.map_or(0, NonZeroU32::get).to_le_bytes());
    out.extend(value.bytes);
    out.extend((value.samples.len() as u32).to_le_bytes());
    for item in &value.samples {
        out.extend(item.to_le_bytes());
    }
    out.extend((value.entries.len() as u32).to_le_bytes());
    for entry in &value.entries {
        out.extend(entry.point.x.to_le_bytes());
        out.extend(entry.point.y.to_le_bytes());
        out.extend(entry.weight.to_le_bytes());
    }
    if version >= 1 {
        out.push(value.point.confidence);
        for entry in &value.entries {
            out.push(entry.point.confidence);
        }
    }
    if version >= 2 {
        out.extend((value.label.len() as u32).to_le_bytes());
        out.extend(value.label.as_bytes());
    }
    prefix_frame(&out)
}

fn historical(bytes: &[u8], version: u32) -> (Sample, usize) {
    let total = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
    let mut input = Cursor::new(&bytes[4..total]);
    let id = input.u64();
    let point = Point {
        x: input.u16(),
        y: input.u16(),
        confidence: 0,
    };
    let tail = input.u32();
    let flags = match input.u8() {
        0 => Some(false),
        1 => Some(true),
        2 => None,
        _ => panic!("option"),
    };
    let ticket = NonZeroU32::new(input.u32());
    let bytes = input.take(8).try_into().unwrap();
    let count = input.u32();
    let samples = (0..count).map(|_| input.u16()).collect();
    let count = input.u32();
    let entries = (0..count)
        .map(|_| Entry {
            point: Point {
                x: input.u16(),
                y: input.u16(),
                confidence: 0,
            },
            weight: input.u32(),
        })
        .collect();
    let mut value = Sample {
        id,
        point,
        tail,
        flags,
        ticket,
        bytes,
        samples,
        entries,
        label: String::new(),
    };
    if version >= 1 && input.remaining() {
        value.point.confidence = input.u8();
        for entry in &mut value.entries {
            entry.point.confidence = input.u8();
        }
    }
    if version >= 2 && input.remaining() {
        value.label = input.string32();
    }
    (value, total)
}

#[test]
fn literal_base_has_one_total_length_and_no_nested_frames() {
    let expected = prefix_frame(&[
        1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 3, 0, 4, 0, 0, 0, 1, 5, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 2,
        0, 0, 0, 0x22, 0x11, 0x44, 0x33, 2, 0, 0, 0, 6, 0, 7, 0, 8, 0, 0, 0, 9, 0, 10, 0, 11, 0, 0,
        0,
    ]);
    assert_eq!(&expected[..4], &61u32.to_le_bytes());
    assert_eq!(wire(&sample(), 0), expected);
    assert_eq!(encode(&sample(), 0), expected);
    assert_eq!(
        Sample::read_version(2, &expected),
        Ok((projected(&sample(), 0), expected.len()))
    );
}

#[test]
fn global_passes_follow_all_base_elements_without_counts() {
    let value = sample();
    let base = wire(&value, 0);
    let v1 = encode(&value, 1);
    assert_eq!(&v1[4..base.len()], &base[4..]);
    assert_eq!(&v1[base.len()..], &[90, 91, 92]);
    let v2 = encode(&value, 2);
    assert_eq!(&v2[4..v1.len()], &v1[4..]);
    assert_eq!(&v2[v1.len()..v1.len() + 4], &7u32.to_le_bytes());
}

#[test]
fn independent_historical_writers_default_later_passes() {
    for writer in 0..=2 {
        let bytes = wire(&sample(), writer);
        assert_eq!(
            Sample::read_version(2, &bytes),
            Ok((projected(&sample(), writer), bytes.len()))
        );
        check_following_sibling(&bytes, projected(&sample(), writer), 2);
    }
}

#[test]
fn independent_historical_readers_ignore_future_passes() {
    for writer in 0..=2 {
        let bytes = encode(&sample(), writer);
        for reader in 0..=2 {
            assert_eq!(
                historical(&bytes, reader),
                (projected(&sample(), writer.min(reader)), bytes.len())
            );
        }
    }
}

#[test]
fn all_writer_reader_version_pairs() {
    assert_eq!(Sample::MAX_VERSION, 2);
    for writer in [0, 1, 2, 3, u32::MAX] {
        let bytes = encode(&sample(), writer);
        assert_eq!(bytes, wire(&sample(), writer));
        for reader in [0, 1, 2, 3, u32::MAX] {
            assert_eq!(
                Sample::read_version(reader, &bytes),
                Ok((projected(&sample(), writer.min(reader)), bytes.len()))
            );
        }
    }
}

#[test]
fn equivalent_macro_values_and_exact_frame_savings() {
    for count in [0, 1, 32, 512, 4096] {
        let mut value = sample();
        value.entries = (0..count)
            .map(|index| Entry {
                point: Point {
                    x: index as u16,
                    y: (index * 3) as u16,
                    confidence: (index % 251) as u8,
                },
                weight: index as u32 + 17,
            })
            .collect();
        for version in 0..=2 {
            let custom = encode(&value, version);
            let derived = encode(&reference(&value), version);
            let expected = projected(&value, version);
            let (decoded, used) = Sample::read_version(version, &custom).unwrap();
            let (macro_decoded, macro_used) = RefSample::read_version(version, &derived).unwrap();
            assert_eq!(decoded, expected);
            assert_eq!(macro_decoded, reference(&expected));
            assert_eq!((used, macro_used), (custom.len(), derived.len()));
            assert_eq!(derived.len() - custom.len(), 4 + 8 * count);
            assert_eq!(
                u32::from_le_bytes(custom[..4].try_into().unwrap()) as usize,
                custom.len()
            );
        }
    }
}

#[test]
fn option_specializations_are_preserved() {
    for flags in [None, Some(false), Some(true)] {
        for ticket in [None, NonZeroU32::new(1), NonZeroU32::new(u32::MAX)] {
            let value = Sample {
                flags,
                ticket,
                ..sample()
            };
            let bytes = wire(&value, 2);
            assert_eq!(encode(&value, 2), bytes);
            assert_eq!(Sample::read_version(2, &bytes), Ok((value, bytes.len())));
        }
    }
}

#[test]
fn truncated_frames_fail_and_reframed_generations_default() {
    for value in [sample(), Sample::default()] {
        for version in 0..=2 {
            let boundaries: Vec<_> = (0..version).map(|v| wire(&value, v).len()).collect();
            check_buffers(&value, version, &boundaries);
        }
    }
}

#[test]
fn a_started_vector_pass_cannot_default_remaining_entries() {
    let mut bytes = wire(&sample(), 0);
    for byte in [90, 91] {
        bytes.push(byte);
        reframe(&mut bytes);
        assert_eq!(Sample::read_version(2, &bytes), Err(InvalidEncoding));
    }
}

#[test]
fn absent_whole_pass_defaults_but_partial_string_fails() {
    let bytes = wire(&sample(), 1);
    assert_eq!(
        Sample::read_version(2, &bytes),
        Ok((projected(&sample(), 1), bytes.len()))
    );
    for partial in [&[7][..], &[7, 0, 0, 0][..], &[7, 0, 0, 0, b'w'][..]] {
        let mut bad = bytes.clone();
        bad.extend(partial);
        reframe(&mut bad);
        assert_eq!(Sample::read_version(2, &bad), Err(InvalidEncoding));
    }
}

#[test]
fn invalid_option_utf8_and_hostile_counts() {
    for (offset, replacement) in [
        (20, vec![3]),
        (33, u32::MAX.to_le_bytes().to_vec()),
        (41, u32::MAX.to_le_bytes().to_vec()),
        (wire(&sample(), 1).len(), u32::MAX.to_le_bytes().to_vec()),
        (wire(&sample(), 1).len() + 4, vec![0xff]),
    ] {
        let mut bytes = wire(&sample(), 2);
        bytes[offset..offset + replacement.len()].copy_from_slice(&replacement);
        assert_eq!(Sample::read_version(2, &bytes), Err(InvalidEncoding));
    }
}

#[test]
fn future_suffix_is_counted_but_following_sibling_is_not() {
    let mut bytes = wire(&sample(), 2);
    bytes.extend([0xff, 0x80, 0, 7]);
    reframe(&mut bytes);
    for version in 0..=2 {
        check_following_sibling(&bytes, projected(&sample(), version), version);
        let mut stream = bytes.clone();
        stream.extend([0xa5; 8]);
        assert_eq!(
            historical(&stream, version),
            (projected(&sample(), version), bytes.len())
        );
    }
}

#[test]
fn generic_vec_and_tuple_preserve_following_values() {
    generic_composition(&[sample(), Sample::default(), sample()], 2);
}

#[test]
fn macro_parent_preserves_trailing_and_versioned_fields() {
    derived_parent_composition(sample());
}

#[test]
fn missing_invalid_and_overflowing_total_lengths_fail() {
    invalid_frame_lengths::<Sample>(&wire(&sample(), 2));
    let mut empty = 4u32.to_le_bytes().to_vec();
    assert_eq!(Sample::read_version(0, &empty), Err(InvalidEncoding));
    empty.extend(wire(&sample(), 0));
    assert_eq!(Sample::read_version(0, &empty), Err(InvalidEncoding));
}

#[test]
fn empty_containers_and_unaligned_input() {
    let value = Sample::default();
    let bytes = wire(&value, 2);
    let mut unaligned = vec![0xa5];
    unaligned.extend(&bytes);
    assert_eq!(
        Sample::read_version(2, &unaligned[1..]),
        Ok((value, bytes.len()))
    );
}

#[test]
fn saved_base_order_survives_distinct_element_values() {
    let mut value = sample();
    value.entries.reverse();
    let bytes = wire(&value, 2);
    assert_eq!(encode(&value, 2), bytes);
    assert_eq!(Sample::read_version(2, &bytes), Ok((value, bytes.len())));
}

#[test]
fn malformed_input_smoke_does_not_panic() {
    malformed_smoke::<Sample>();
}
