use custom_vercodable_eval::{Block, Code, Extension, Frame, Mode, Strict};

include!("skill_eval_support/common.rs");

type Custom = Frame;

fn frame_layout(_: &[u8]) -> (usize, usize) {
    (0, 0)
}

fn frame() -> Frame {
    Frame {
        mode: Mode::Named {
            code: 0x1020_3040,
            extra: 0x1234,
        },
        strict: Strict::Value(5),
        tail: 0x5566_7788,
        block: Block {
            a: 0x1234_5678,
            b: 0x90ab_cdef,
        },
        code: Code::Ready,
        modes: vec![
            Mode::Idle,
            Mode::Named { code: 6, extra: 7 },
            Mode::Pair(8, 9),
            Mode::Named {
                code: 10,
                extra: 11,
            },
        ],
        extension: Some(Extension {
            modes: vec![
                Mode::Named {
                    code: 12,
                    extra: 13,
                },
                Mode::Added([14, 15, 16]),
            ],
            checksum: 17,
        }),
    }
}

fn mode_birth(mode: &Mode, birth: u32, _version: u32, out: &mut Vec<u8>) {
    match mode {
        Mode::Unknown => out.extend(0u16.to_le_bytes()),
        Mode::Idle => out.extend(1u16.to_le_bytes()),
        Mode::Pair(a, b) => {
            out.extend(2u16.to_le_bytes());
            out.extend(a.to_le_bytes());
            out.extend(b.to_le_bytes());
        }
        Mode::Named { code, extra } => {
            out.extend(3u16.to_le_bytes());
            out.extend(code.to_le_bytes());
            if birth >= 1 {
                out.extend(extra.to_le_bytes());
            }
        }
        Mode::Added(bytes) => {
            out.extend(4u16.to_le_bytes());
            if birth >= 2 {
                out.extend(bytes);
            }
        }
    }
}

fn wire(value: &Frame, version: u32) -> Vec<u8> {
    let mut out = Vec::new();
    mode_birth(&value.mode, 0, version, &mut out);
    match value.strict {
        Strict::Idle => out.extend(0u16.to_le_bytes()),
        Strict::Value(number) => {
            out.extend(1u16.to_le_bytes());
            out.extend(number.to_le_bytes());
        }
    }
    out.extend(value.tail.to_le_bytes());
    out.extend(value.block.a.to_le_bytes());
    out.extend(value.block.b.to_le_bytes());
    out.push(value.code as u8);
    out.extend((value.modes.len() as u32).to_le_bytes());
    for mode in &value.modes {
        mode_birth(mode, 0, version, &mut out);
    }
    if version >= 1 {
        for mode in std::iter::once(&value.mode).chain(&value.modes) {
            if let Mode::Named { extra, .. } = mode {
                out.extend(extra.to_le_bytes());
            }
        }
    }
    if version >= 2 {
        for mode in std::iter::once(&value.mode).chain(&value.modes) {
            if let Mode::Added(bytes) = mode {
                out.extend(bytes);
            }
        }
        out.push(u8::from(value.extension.is_some()));
        if let Some(extension) = &value.extension {
            out.extend((extension.modes.len() as u32).to_le_bytes());
            for mode in &extension.modes {
                mode_birth(mode, 2, version, &mut out);
            }
        }
    }
    if version >= 3
        && let Some(extension) = &value.extension
    {
        out.extend(extension.checksum.to_le_bytes());
    }
    prefix_frame(&out)
}

fn projected(value: &Frame, version: u32) -> Frame {
    let mut value = value.clone();
    if version < 1 {
        for mode in std::iter::once(&mut value.mode).chain(&mut value.modes) {
            if let Mode::Named { extra, .. } = mode {
                *extra = 0;
            }
        }
    }
    if version < 2 {
        for mode in std::iter::once(&mut value.mode).chain(&mut value.modes) {
            if matches!(mode, Mode::Added(_)) {
                *mode = Mode::Unknown;
            }
        }
        value.extension = None;
    }
    if version < 3
        && let Some(extension) = &mut value.extension
    {
        extension.checksum = 0;
    }
    value
}

fn read_mode(input: &mut Cursor<'_>, birth: u32, version: u32) -> Mode {
    match input.u16() {
        0 => Mode::Unknown,
        1 => Mode::Idle,
        2 => Mode::Pair(input.u16(), input.u32()),
        3 => Mode::Named {
            code: input.u32(),
            extra: if birth >= 1 { input.u16() } else { 0 },
        },
        4 if version >= 2 => Mode::Added(if birth >= 2 {
            input.take(3).try_into().unwrap()
        } else {
            [0; 3]
        }),
        _ => Mode::Unknown,
    }
}

fn historical(bytes: &[u8], version: u32) -> (Frame, usize) {
    let total = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
    let mut input = Cursor::new(&bytes[4..total]);
    let mode = read_mode(&mut input, 0, version);
    let strict = match input.u16() {
        0 => Strict::Idle,
        1 => Strict::Value(input.u16()),
        _ => panic!("strict"),
    };
    let tail = input.u32();
    let block = Block {
        a: input.u32(),
        b: input.u32(),
    };
    let code = match input.u8() {
        7 => Code::Ready,
        13 => Code::Busy,
        _ => Code::Unknown,
    };
    let count = input.u32();
    let modes = (0..count)
        .map(|_| read_mode(&mut input, 0, version))
        .collect();
    let mut value = Frame {
        mode,
        strict,
        tail,
        block,
        code,
        modes,
        extension: None,
    };
    if version >= 1 && input.remaining() {
        for mode in std::iter::once(&mut value.mode).chain(&mut value.modes) {
            if let Mode::Named { extra, .. } = mode {
                *extra = input.u16();
            }
        }
    }
    if version >= 2 && input.remaining() {
        for mode in std::iter::once(&mut value.mode).chain(&mut value.modes) {
            if let Mode::Added(bytes) = mode {
                *bytes = input.take(3).try_into().unwrap();
            }
        }
        value.extension = match input.u8() {
            0 => None,
            1 => {
                let count = input.u32();
                Some(Extension {
                    modes: (0..count)
                        .map(|_| read_mode(&mut input, 2, version))
                        .collect(),
                    checksum: 0,
                })
            }
            _ => panic!("option"),
        };
    }
    if version >= 3
        && input.remaining()
        && let Some(extension) = &mut value.extension
    {
        extension.checksum = input.u32();
    }
    (value, total)
}

fn mode_length(value: &Frame) -> usize {
    let mut bytes = Vec::new();
    mode_birth(&value.mode, 0, 3, &mut bytes);
    bytes.len()
}

fn code_offset(value: &Frame) -> usize {
    4 + mode_length(value)
        + match value.strict {
            Strict::Idle => 2,
            Strict::Value(_) => 4,
        }
        + 12
}

#[test]
fn literal_base_has_one_total_length_and_no_nested_frames() {
    let value = Frame {
        mode: Mode::Pair(0x1234, 0x5678_9abc),
        strict: Strict::Value(0xabcd),
        tail: 0x1020_3040,
        block: Block {
            a: 0x1234_5678,
            b: 0x90ab_cdef,
        },
        code: Code::Busy,
        modes: Vec::new(),
        extension: None,
    };
    let expected = prefix_frame(&[
        2, 0, 0x34, 0x12, 0xbc, 0x9a, 0x78, 0x56, 1, 0, 0xcd, 0xab, 0x40, 0x30, 0x20, 0x10, 0x78,
        0x56, 0x34, 0x12, 0xef, 0xcd, 0xab, 0x90, 13, 0, 0, 0, 0,
    ]);
    assert_eq!(&expected[..4], &33u32.to_le_bytes());
    assert_eq!(wire(&value, 0), expected);
    assert_eq!(encode(&value, 0), expected);
    assert_eq!(
        Frame::read_version(3, &expected),
        Ok((value, expected.len()))
    );
}

#[test]
fn named_fields_follow_the_complete_base_vector() {
    let value = frame();
    let base = wire(&value, 0);
    let one = encode(&value, 1);
    assert_eq!(&one[4..base.len()], &base[4..]);
    assert_eq!(&one[base.len()..], &[0x34, 0x12, 7, 0, 11, 0]);
    assert_eq!(historical(&one, 0), (projected(&value, 0), one.len()));
}

#[test]
fn new_subtree_birth_includes_earlier_descendant_fields_once() {
    let value = frame();
    let one = wire(&value, 1);
    let two = encode(&value, 2);
    assert_eq!(&two[4..one.len()], &one[4..]);
    assert_eq!(
        &two[one.len()..],
        &[1, 2, 0, 0, 0, 3, 0, 12, 0, 0, 0, 13, 0, 4, 0, 14, 15, 16,]
    );
    let three = encode(&value, 3);
    assert_eq!(&three[4..two.len()], &two[4..]);
    assert_eq!(&three[two.len()..], &[17, 0, 0, 0]);
}

#[test]
fn independent_historical_writers_and_readers_all_versions() {
    assert_eq!(Frame::MAX_VERSION, 3);
    for writer in [0, 1, 2, 3, 4, u32::MAX] {
        let expected_bytes = wire(&frame(), writer);
        assert_eq!(encode(&frame(), writer), expected_bytes);
        for reader in [0, 1, 2, 3, 4, u32::MAX] {
            let expected = projected(&frame(), writer.min(reader));
            assert_eq!(
                historical(&expected_bytes, reader),
                (expected.clone(), expected_bytes.len())
            );
            assert_eq!(
                Frame::read_version(reader, &expected_bytes),
                Ok((expected, expected_bytes.len()))
            );
            check_following_sibling(
                &expected_bytes,
                projected(&frame(), writer.min(reader)),
                reader,
            );
        }
    }
}

#[test]
fn known_base_variants_and_strict_values() {
    for mode in [
        Mode::Unknown,
        Mode::Idle,
        Mode::Pair(7, 9),
        Mode::Named {
            code: 11,
            extra: 13,
        },
    ] {
        for strict in [Strict::Idle, Strict::Value(0xabcd)] {
            let value = Frame {
                mode: mode.clone(),
                strict,
                ..frame()
            };
            for version in 0..=3 {
                let bytes = wire(&value, version);
                assert_eq!(encode(&value, version), bytes);
                assert_eq!(
                    Frame::read_version(version, &bytes),
                    Ok((projected(&value, version), bytes.len()))
                );
            }
        }
    }
}

#[test]
fn added_variant_earlier_writes_reject_but_earlier_reads_default() {
    for in_vector in [false, true] {
        let mut value = frame();
        if in_vector {
            value.modes[0] = Mode::Added([1, 2, 3]);
        } else {
            value.mode = Mode::Added([1, 2, 3]);
        }
        for version in [0, 1] {
            assert_eq!(value.size_version(version), Err(InvalidEncoding));
            assert_eq!(
                value.write_version(version, &mut [0; 1024]),
                Err(InvalidEncoding)
            );
            let bytes = wire(&value, 3);
            assert_eq!(
                historical(&bytes, version),
                (projected(&value, version), bytes.len())
            );
            assert_eq!(
                Frame::read_version(version, &bytes),
                Ok((projected(&value, version), bytes.len()))
            );
        }
        for version in [2, 3] {
            let bytes = wire(&value, version);
            assert_eq!(encode(&value, version), bytes);
            assert_eq!(
                Frame::read_version(version, &bytes),
                Ok((projected(&value, version), bytes.len()))
            );
        }
    }
}

#[test]
fn unknown_future_tags_before_siblings_have_no_known_pass_payload() {
    let value = Frame {
        mode: Mode::Unknown,
        ..frame()
    };
    for offset in [4, code_offset(&value) + 1 + 4] {
        let mut bytes = wire(&value, 3);
        bytes[offset..offset + 2].copy_from_slice(&0xfffeu16.to_le_bytes());
        // The unknown variant's future payload is opaque and occurs only after
        // every known pass, not beside the tag in the base traversal.
        bytes.extend([0xff, 0x80, 7, 9]);
        reframe(&mut bytes);
        let mut expected = value.clone();
        if offset != 4 {
            expected.modes[0] = Mode::Unknown;
        }
        for version in 0..=3 {
            assert_eq!(
                historical(&bytes, version),
                (projected(&expected, version), bytes.len())
            );
            assert_eq!(
                Frame::read_version(version, &bytes),
                Ok((projected(&expected, version), bytes.len()))
            );
        }
    }
}

#[test]
fn added_payloads_are_deferred_after_all_base_and_named_fields() {
    let mut value = Frame {
        mode: Mode::Added([21, 22, 23]),
        ..frame()
    };
    value.modes[0] = Mode::Added([31, 32, 33]);
    let base = wire(&value, 0);
    assert_eq!(&base[4..6], &[4, 0]);
    assert_eq!(&base[6..10], &[1, 0, 5, 0]);
    let one = wire(&value, 1);
    let two = encode(&value, 2);
    assert_eq!(&two[4..one.len()], &one[4..]);
    assert_eq!(&two[one.len()..one.len() + 6], &[21, 22, 23, 31, 32, 33]);
    for boundary in [base.len(), one.len()] {
        let bytes = resized_frame(&two, boundary);
        let (expected, used) = historical(&bytes, 3);
        assert_eq!(used, bytes.len());
        assert_eq!(expected.mode, Mode::Added([0; 3]));
        assert_eq!(expected.modes[0], Mode::Added([0; 3]));
        assert_eq!(Frame::read_version(3, &bytes), Ok((expected, bytes.len())));
    }
}

#[test]
fn added_started_pass_requires_all_payloads_and_new_shape() {
    let mut value = Frame {
        mode: Mode::Added([21, 22, 23]),
        ..frame()
    };
    value.modes[0] = Mode::Added([31, 32, 33]);
    let start = wire(&value, 1).len();
    let two = wire(&value, 2);
    for end in start + 1..two.len() {
        assert_eq!(
            Frame::read_version(3, &resized_frame(&two, end)),
            Err(InvalidEncoding)
        );
    }
    let boundaries = [wire(&value, 0).len(), start, two.len()];
    check_buffers(&value, 3, &boundaries);
}

#[test]
fn unknown_strict_tag_is_not_defaulted() {
    let mut bytes = wire(&frame(), 3);
    let offset = 4 + mode_length(&frame());
    bytes[offset..offset + 2].copy_from_slice(&0xffffu16.to_le_bytes());
    assert_eq!(Frame::read_version(3, &bytes), Err(InvalidEncoding));
}

#[test]
fn unknown_complete_code_defaults_and_preserves_vector_siblings() {
    for code in [1, 6, 14, 255] {
        let mut bytes = wire(&frame(), 3);
        bytes[code_offset(&frame())] = code;
        let expected = Frame {
            code: Code::Unknown,
            ..frame()
        };
        assert_eq!(Frame::read_version(3, &bytes), Ok((expected, bytes.len())));
    }
    for code in [Code::Unknown, Code::Ready, Code::Busy] {
        let value = Frame { code, ..frame() };
        assert_eq!(encode(&value, 3), wire(&value, 3));
    }
}

#[test]
fn malformed_known_payloads_and_missing_codes_never_default() {
    let bytes = wire(&frame(), 3);
    for end in [
        0,
        1,
        2,
        3,
        4,
        5,
        6,
        7,
        8,
        9,
        mode_length(&frame()) + 7,
        code_offset(&frame()),
    ] {
        assert_eq!(Frame::read_version(3, &bytes[..end]), Err(InvalidEncoding));
        if end >= 4 {
            assert_eq!(
                Frame::read_version(3, &resized_frame(&bytes, end)),
                Err(InvalidEncoding)
            );
        }
    }
    let value = Frame {
        mode: Mode::Pair(7, 8),
        ..frame()
    };
    for end in 6..12 {
        assert_eq!(
            Frame::read_version(3, &resized_frame(&wire(&value, 3), end)),
            Err(InvalidEncoding)
        );
    }
}

#[test]
fn started_named_pass_cannot_default_later_vector_entries() {
    let base = wire(&frame(), 0).len();
    let one = wire(&frame(), 1);
    for extra in 1..6 {
        assert_eq!(
            Frame::read_version(3, &resized_frame(&one, base + extra)),
            Err(InvalidEncoding)
        );
    }
}

#[test]
fn zero_byte_pass_does_not_hide_nonempty_later_passes() {
    let value = Frame {
        mode: Mode::Idle,
        modes: vec![Mode::Idle, Mode::Pair(1, 2)],
        ..frame()
    };
    assert_eq!(wire(&value, 0), wire(&value, 1));
    for version in 0..=3 {
        let bytes = wire(&value, version);
        assert_eq!(encode(&value, version), bytes);
        assert_eq!(
            Frame::read_version(3, &bytes),
            Ok((projected(&value, version), bytes.len()))
        );
    }
    let boundaries: Vec<_> = (0..3).map(|v| wire(&value, v).len()).collect();
    check_buffers(&value, 3, &boundaries);
}

#[test]
fn absent_extension_has_a_zero_byte_checksum_pass() {
    let value = Frame {
        extension: None,
        ..frame()
    };
    assert_eq!(wire(&value, 2), wire(&value, 3));
    let bytes = encode(&value, 3);
    assert_eq!(bytes, wire(&value, 3));
    assert_eq!(Frame::read_version(3, &bytes), Ok((value, bytes.len())));
}

#[test]
fn truncated_frames_fail_and_reframed_generations_default() {
    for value in [
        frame(),
        Frame {
            extension: Some(Extension::default()),
            ..frame()
        },
    ] {
        for version in 0..=3 {
            let boundaries: Vec<_> = (0..version).map(|v| wire(&value, v).len()).collect();
            check_buffers(&value, version, &boundaries);
        }
    }
}

#[test]
fn invalid_option_and_hostile_counts_fail() {
    let value = frame();
    let birth = wire(&value, 1).len();
    for (offset, replacement) in [
        (birth, vec![2]),
        (code_offset(&value) + 1, u32::MAX.to_le_bytes().to_vec()),
        (birth + 1, u32::MAX.to_le_bytes().to_vec()),
    ] {
        let mut bytes = wire(&value, 3);
        bytes[offset..offset + replacement.len()].copy_from_slice(&replacement);
        assert_eq!(Frame::read_version(3, &bytes), Err(InvalidEncoding));
    }
}

#[test]
fn unknown_future_tag_in_new_subtree_preserves_known_birth_fields() {
    let value = Frame {
        extension: Some(Extension {
            modes: vec![Mode::Unknown, Mode::Named { code: 4, extra: 5 }],
            checksum: 6,
        }),
        ..frame()
    };
    let mut bytes = wire(&value, 3);
    let offset = wire(&value, 1).len() + 5;
    bytes[offset..offset + 2].copy_from_slice(&0xfffdu16.to_le_bytes());
    bytes.extend([0xff, 0x80]);
    reframe(&mut bytes);
    assert_eq!(Frame::read_version(3, &bytes), Ok((value, bytes.len())));
}

#[test]
fn block_has_exact_unframed_le_bytes_and_accepts_unaligned_input() {
    let value = frame();
    let bytes = wire(&value, 3);
    let start = code_offset(&value) - 8;
    assert_eq!(
        &bytes[start..start + 8],
        &[0x78, 0x56, 0x34, 0x12, 0xef, 0xcd, 0xab, 0x90]
    );
    let mut unaligned = vec![0xff];
    unaligned.extend(&bytes);
    assert_eq!(
        Frame::read_version(3, &unaligned[1..]),
        Ok((value, bytes.len()))
    );
}

#[test]
fn future_suffix_is_counted_but_following_sibling_is_not() {
    let mut bytes = wire(&frame(), 3);
    bytes.extend([0xff, 0xfe, 0x80, 7]);
    reframe(&mut bytes);
    for version in 0..=3 {
        check_following_sibling(&bytes, projected(&frame(), version), version);
        let mut stream = bytes.clone();
        stream.extend([0xa5; 7]);
        assert_eq!(
            historical(&stream, version),
            (projected(&frame(), version), bytes.len())
        );
    }
}

#[test]
fn generic_vec_and_tuple_preserve_following_values() {
    generic_composition(
        &[
            frame(),
            Frame {
                extension: None,
                ..frame()
            },
            frame(),
        ],
        3,
    );
}

#[test]
fn macro_parent_preserves_trailing_and_versioned_fields() {
    derived_parent_composition(frame());
}

#[test]
fn missing_invalid_and_overflowing_total_lengths_fail() {
    invalid_frame_lengths::<Frame>(&wire(&frame(), 3));
    let bytes = 4u32.to_le_bytes();
    assert_eq!(Frame::read_version(0, &bytes), Err(InvalidEncoding));
}

#[test]
fn saved_enum_topology_controls_replay_order() {
    let mut value = frame();
    value.modes.reverse();
    let bytes = wire(&value, 3);
    assert_eq!(encode(&value, 3), bytes);
    assert_eq!(Frame::read_version(3, &bytes), Ok((value, bytes.len())));
}

#[test]
fn malformed_input_smoke_does_not_panic() {
    malformed_smoke::<Frame>();
}
