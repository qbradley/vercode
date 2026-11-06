// Copyright (c) Microsoft Corporation. All rights reserved.
use std::hint::black_box;
use std::time::Instant;

use vercode::{InvalidEncoding, Vercode, WireField};

const SAMPLES: usize = 9;
const MAX_MEDIAN_RATIO: f64 = 1.05;
const SMALL_SIZE: usize = 24;
const BLOCKS: usize = 8;
const CHECKSUMS: usize = 16;
const BLOCK_SIZE: usize = 4 + 2 + 4 + CHECKSUMS * 8;
const NESTED_SIZE: usize = 4 + BLOCKS * BLOCK_SIZE + 8;

#[derive(Clone, Debug, PartialEq)]
struct Small {
    offset: u64,
    file_id: u128,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Vercode)]
struct Checksum(u32);

#[derive(Clone, Debug, Default, PartialEq, Vercode)]
struct Block {
    lane: u16,
    epoch: u32,
    checksums: [Checksum; CHECKSUMS],
}

#[derive(Clone, Debug, Default, PartialEq, Vercode)]
struct Nested {
    blocks: [Block; BLOCKS],
    marker: u64,
}

mod small_fields {
    use vercode::WireField;
    pub const OFFSET: WireField<u64, 0> = WireField::new();
    pub const FILE_ID: WireField<u128, 8> = WireField::new();
}

mod block_fields {
    use vercode::WireField;
    pub const LENGTH: WireField<u32, 0> = WireField::new();
    pub const LANE: WireField<u16, 4> = WireField::new();
    pub const EPOCH: WireField<u32, 6> = WireField::new();
}

const ROOT_LENGTH: WireField<u32, 0> = WireField::new();
const MARKER: WireField<u64, { NESTED_SIZE - 8 }> = WireField::new();
const CHECKSUM_LENGTH: WireField<u32, 0> = WireField::new();
const CHECKSUM_VALUE: WireField<u32, 4> = WireField::new();
const PACKED_CHECKSUM: WireField<u64, 0> = WireField::new();

#[inline(never)]
fn write_small_manual(value: &Small, buf: &mut [u8]) -> Result<usize, InvalidEncoding> {
    let run = buf.get_mut(..SMALL_SIZE).ok_or(InvalidEncoding)?;
    run[..8].copy_from_slice(&value.offset.to_le_bytes());
    run[8..24].copy_from_slice(&value.file_id.to_le_bytes());
    Ok(SMALL_SIZE)
}

#[inline(never)]
fn write_small_fields(value: &Small, buf: &mut [u8]) -> Result<usize, InvalidEncoding> {
    let run = buf.get_mut(..SMALL_SIZE).ok_or(InvalidEncoding)?;
    small_fields::OFFSET.write(run, value.offset)?;
    small_fields::FILE_ID.write(run, value.file_id)?;
    Ok(SMALL_SIZE)
}

#[inline(never)]
fn read_small_manual(buf: &[u8]) -> Result<Small, InvalidEncoding> {
    let run = buf.get(..SMALL_SIZE).ok_or(InvalidEncoding)?;
    Ok(Small {
        offset: u64::from_le_bytes(run[..8].try_into().map_err(|_| InvalidEncoding)?),
        file_id: u128::from_le_bytes(run[8..24].try_into().map_err(|_| InvalidEncoding)?),
    })
}

#[inline(never)]
fn read_small_fields(buf: &[u8]) -> Result<Small, InvalidEncoding> {
    let run = buf.get(..SMALL_SIZE).ok_or(InvalidEncoding)?;
    Ok(Small {
        offset: small_fields::OFFSET.read(run)?,
        file_id: small_fields::FILE_ID.read(run)?,
    })
}

#[inline(never)]
fn size_small_manual(_: &Small) -> usize {
    SMALL_SIZE
}

#[inline(never)]
fn size_small_fields(_: &Small) -> usize {
    SMALL_SIZE
}

#[inline(never)]
fn write_nested_manual(value: &Nested, buf: &mut [u8]) -> Result<usize, InvalidEncoding> {
    let frame = buf.get_mut(..NESTED_SIZE).ok_or(InvalidEncoding)?;
    for (block, run) in value
        .blocks
        .iter()
        .zip(frame[4..NESTED_SIZE - 8].as_chunks_mut::<BLOCK_SIZE>().0)
    {
        run[..4].copy_from_slice(&(BLOCK_SIZE as u32).to_le_bytes());
        run[4..6].copy_from_slice(&block.lane.to_le_bytes());
        run[6..10].copy_from_slice(&block.epoch.to_le_bytes());
        for (checksum, chunk) in block.checksums.iter().zip(run[10..].as_chunks_mut::<8>().0) {
            let packed = 8u64 | (u64::from(checksum.0) << 32);
            chunk.copy_from_slice(&packed.to_le_bytes());
        }
    }
    frame[NESTED_SIZE - 8..].copy_from_slice(&value.marker.to_le_bytes());
    frame[..4].copy_from_slice(&(NESTED_SIZE as u32).to_le_bytes());
    Ok(NESTED_SIZE)
}

#[inline(never)]
fn write_nested_fields(value: &Nested, buf: &mut [u8]) -> Result<usize, InvalidEncoding> {
    let frame = buf.get_mut(..NESTED_SIZE).ok_or(InvalidEncoding)?;
    for (block, run) in value
        .blocks
        .iter()
        .zip(frame[4..NESTED_SIZE - 8].as_chunks_mut::<BLOCK_SIZE>().0)
    {
        block_fields::LENGTH.write(run, BLOCK_SIZE as u32)?;
        block_fields::LANE.write(run, block.lane)?;
        block_fields::EPOCH.write(run, block.epoch)?;
        for (checksum, chunk) in block.checksums.iter().zip(run[10..].as_chunks_mut::<8>().0) {
            let packed = 8u64 | (u64::from(checksum.0) << 32);
            PACKED_CHECKSUM.write(chunk, packed)?;
        }
    }
    MARKER.write(frame, value.marker)?;
    ROOT_LENGTH.write(frame, NESTED_SIZE as u32)?;
    Ok(NESTED_SIZE)
}

#[inline(never)]
fn read_nested_manual(buf: &[u8]) -> Result<Nested, InvalidEncoding> {
    let frame = buf.get(..NESTED_SIZE).ok_or(InvalidEncoding)?;
    if u32::from_le_bytes(frame[..4].try_into().map_err(|_| InvalidEncoding)?) != NESTED_SIZE as u32
    {
        return Err(InvalidEncoding);
    }
    let mut result = Nested::default();
    for (block, run) in result
        .blocks
        .iter_mut()
        .zip(frame[4..NESTED_SIZE - 8].as_chunks::<BLOCK_SIZE>().0)
    {
        if u32::from_le_bytes(run[..4].try_into().map_err(|_| InvalidEncoding)?)
            != BLOCK_SIZE as u32
        {
            return Err(InvalidEncoding);
        }
        block.lane = u16::from_le_bytes(run[4..6].try_into().map_err(|_| InvalidEncoding)?);
        block.epoch = u32::from_le_bytes(run[6..10].try_into().map_err(|_| InvalidEncoding)?);
        for (checksum, chunk) in block.checksums.iter_mut().zip(run[10..].as_chunks::<8>().0) {
            if u32::from_le_bytes(chunk[..4].try_into().map_err(|_| InvalidEncoding)?) != 8 {
                return Err(InvalidEncoding);
            }
            checksum.0 = u32::from_le_bytes(chunk[4..8].try_into().map_err(|_| InvalidEncoding)?);
        }
    }
    result.marker = u64::from_le_bytes(
        frame[NESTED_SIZE - 8..]
            .try_into()
            .map_err(|_| InvalidEncoding)?,
    );
    Ok(result)
}

#[inline(never)]
fn read_nested_fields(buf: &[u8]) -> Result<Nested, InvalidEncoding> {
    let frame = buf.get(..NESTED_SIZE).ok_or(InvalidEncoding)?;
    if ROOT_LENGTH.read(frame)? != NESTED_SIZE as u32 {
        return Err(InvalidEncoding);
    }
    let mut result = Nested::default();
    for (block, run) in result
        .blocks
        .iter_mut()
        .zip(frame[4..NESTED_SIZE - 8].as_chunks::<BLOCK_SIZE>().0)
    {
        if block_fields::LENGTH.read(run)? != BLOCK_SIZE as u32 {
            return Err(InvalidEncoding);
        }
        block.lane = block_fields::LANE.read(run)?;
        block.epoch = block_fields::EPOCH.read(run)?;
        for (checksum, chunk) in block.checksums.iter_mut().zip(run[10..].as_chunks::<8>().0) {
            if CHECKSUM_LENGTH.read(chunk)? != 8 {
                return Err(InvalidEncoding);
            }
            checksum.0 = CHECKSUM_VALUE.read(chunk)?;
        }
    }
    result.marker = MARKER.read(frame)?;
    Ok(result)
}

#[inline(never)]
fn size_nested_manual(_: &Nested) -> usize {
    NESTED_SIZE
}

#[inline(never)]
fn size_nested_fields(_: &Nested) -> usize {
    NESTED_SIZE
}

// Both paths use the same timing loop and state, including the output address.
#[inline(never)]
fn measure<C, T>(operation: fn(&mut C) -> T, state: &mut C, iterations: usize) -> f64 {
    let operation = black_box(operation);
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(operation(black_box(&mut *state)));
    }
    start.elapsed().as_nanos() as f64 / iterations as f64
}

fn median(values: &[f64; SAMPLES]) -> f64 {
    let mut sorted = *values;
    sorted.sort_by(f64::total_cmp);
    sorted[SAMPLES / 2]
}

fn paired<C, T>(
    workload: &str,
    method: &str,
    state: &mut C,
    manual: fn(&mut C) -> T,
    fields: fn(&mut C) -> T,
    iterations: usize,
) -> bool {
    measure(manual, state, 10_000);
    measure(fields, state, 10_000);
    let mut manual_ns = [0.0; SAMPLES];
    let mut fields_ns = [0.0; SAMPLES];
    for sample in 0..SAMPLES {
        if sample % 2 == 0 {
            manual_ns[sample] = measure(manual, state, iterations);
            fields_ns[sample] = measure(fields, state, iterations);
        } else {
            fields_ns[sample] = measure(fields, state, iterations);
            manual_ns[sample] = measure(manual, state, iterations);
        }
    }
    let ratio = median(&fields_ns) / median(&manual_ns);
    let passed = ratio <= MAX_MEDIAN_RATIO;
    println!(
        "{{\"workload\":\"{workload}\",\"method\":\"{method}\",\"iterations\":{iterations},\
         \"manual_ns\":{manual_ns:?},\"fields_ns\":{fields_ns:?},\
         \"median_ratio\":{ratio},\"passed\":{passed}}}"
    );
    passed
}

fn main() {
    // Expose the behavior checks as one non-ignored test to libtest-compatible runners.
    if std::env::args().any(|arg| arg == "--ignored") {
        return;
    }
    if std::env::args().any(|arg| arg == "--list") {
        println!("wire_fields_behavior: test");
        return;
    }

    let small = Small {
        offset: 0x0123_4567_89ab_cdef,
        file_id: u128::MAX - 42,
    };
    let nested = Nested {
        blocks: std::array::from_fn(|i| Block {
            lane: (i * 3) as u16,
            epoch: (i * 257 + 13) as u32,
            checksums: std::array::from_fn(|j| Checksum((i * 4099 + j * 131) as u32)),
        }),
        marker: 0x8877_6655_4433_2211,
    };
    let small_bytes = [
        0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01, 0xd5, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    ];
    let nested_bytes = vercode::serialize_to_vec(&nested).unwrap();
    assert_eq!(nested_bytes.len(), NESTED_SIZE);
    for write in [write_small_manual, write_small_fields] {
        let mut output = vec![0; SMALL_SIZE];
        assert_eq!(write(&small, &mut output), Ok(SMALL_SIZE));
        assert_eq!(output, small_bytes);
        for length in 0..SMALL_SIZE {
            assert_eq!(write(&small, &mut output[..length]), Err(InvalidEncoding));
        }
    }
    for write in [write_nested_manual, write_nested_fields] {
        let mut output = vec![0; NESTED_SIZE];
        assert_eq!(write(&nested, &mut output), Ok(NESTED_SIZE));
        assert_eq!(output, nested_bytes);
        for length in 0..NESTED_SIZE {
            assert_eq!(write(&nested, &mut output[..length]), Err(InvalidEncoding));
        }
    }
    for read in [read_small_manual, read_small_fields] {
        assert_eq!(read(&small_bytes), Ok(small.clone()));
        for length in 0..SMALL_SIZE {
            assert_eq!(read(&small_bytes[..length]), Err(InvalidEncoding));
        }
    }
    for read in [read_nested_manual, read_nested_fields] {
        assert_eq!(read(&nested_bytes), Ok(nested.clone()));
        for length in 0..NESTED_SIZE {
            assert_eq!(read(&nested_bytes[..length]), Err(InvalidEncoding));
        }
        for offset in [0, 4, 14] {
            let mut invalid = nested_bytes.clone();
            invalid[offset] = 0;
            assert_eq!(read(&invalid), Err(InvalidEncoding));
        }
    }
    assert_eq!(size_small_manual(&small), small_bytes.len());
    assert_eq!(size_small_fields(&small), small_bytes.len());
    assert_eq!(size_nested_manual(&nested), nested_bytes.len());
    assert_eq!(size_nested_fields(&nested), nested_bytes.len());
    if !std::env::args().any(|arg| arg == "--measure") {
        println!("Wire-field benchmark behavior passed; use --measure to collect timings.");
        return;
    }
    let iterations = std::env::var("WIRE_FIELD_BENCH_ITERATIONS")
        .map(|value| value.parse::<usize>().expect("positive iteration count"))
        .unwrap_or(1_000_000);
    assert!(iterations > 0);
    println!(
        "{{\"samples\":{SAMPLES},\"maximum_median_ratio\":{MAX_MEDIAN_RATIO},\
         \"small_bytes\":{SMALL_SIZE},\"nested_bytes\":{NESTED_SIZE}}}"
    );
    let mut output = vec![0; NESTED_SIZE];
    let mut passed = paired(
        "small",
        "write",
        &mut (&small, output.as_mut_slice()),
        |state| write_small_manual(black_box(state.0), black_box(state.1)).unwrap(),
        |state| write_small_fields(black_box(state.0), black_box(state.1)).unwrap(),
        iterations,
    );
    passed &= paired(
        "small",
        "read_owned",
        &mut small_bytes.as_slice(),
        |input| read_small_manual(black_box(input)).unwrap(),
        |input| read_small_fields(black_box(input)).unwrap(),
        iterations,
    );
    passed &= paired(
        "small",
        "size",
        &mut &small,
        |value| size_small_manual(black_box(value)),
        |value| size_small_fields(black_box(value)),
        iterations,
    );
    passed &= paired(
        "nested",
        "write",
        &mut (&nested, output.as_mut_slice()),
        |state| write_nested_manual(black_box(state.0), black_box(state.1)).unwrap(),
        |state| write_nested_fields(black_box(state.0), black_box(state.1)).unwrap(),
        iterations,
    );
    passed &= paired(
        "nested",
        "read_owned",
        &mut nested_bytes.as_slice(),
        |input| read_nested_manual(black_box(input)).unwrap(),
        |input| read_nested_fields(black_box(input)).unwrap(),
        iterations,
    );
    passed &= paired(
        "nested",
        "size",
        &mut &nested,
        |value| size_nested_manual(black_box(value)),
        |value| size_nested_fields(black_box(value)),
        iterations,
    );
    assert!(
        passed,
        "wire-field median non-regression gate failed; retain these samples"
    );
}
