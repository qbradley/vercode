use std::fmt::Debug;
use vercode::{InvalidEncoding, VerCodable};

fn encode<T: VerCodable>(value: &T, version: u32) -> Vec<u8> {
    let size = value.size_version(version).unwrap();
    let mut bytes = vec![0; size];
    assert_eq!(value.write_version(version, &mut bytes), Ok(size));
    bytes
}

fn check_buffers<T: VerCodable + Debug>(value: &T, version: u32, boundaries: &[usize]) {
    let bytes = encode(value, version);
    for len in 0..bytes.len() {
        let mut output = vec![0xa5; len];
        assert_eq!(
            value.write_version(version, &mut output),
            Err(InvalidEncoding),
            "accepted output length {len}"
        );
        assert!(
            T::read_version(version, &bytes[..len]).is_err(),
            "input length {len}: a declared frame cannot be truncated"
        );
        let (header, _) = frame_layout(&bytes);
        if len >= header + 4 {
            let historical = resized_frame(&bytes, len);
            assert_eq!(
                T::read_version(version, &historical).is_ok(),
                boundaries.contains(&len),
                "re-framed input length {len}: only complete global generations may default"
            );
        }
    }
    let mut output = vec![0xa5; bytes.len() + 8];
    assert_eq!(value.write_version(version, &mut output), Ok(bytes.len()));
    assert_eq!(&output[..bytes.len()], bytes);
    assert_eq!(&output[bytes.len()..], &[0xa5; 8]);
}

// frame_layout returns the header offset and the start of its counted extent.
// Prefix lengths count themselves; adoption trailers count only extensions.
fn reframe(bytes: &mut [u8]) {
    let (header, start) = frame_layout(bytes);
    let length = u32::try_from(bytes.len() - start).unwrap();
    bytes[header..header + 4].copy_from_slice(&length.to_le_bytes());
}

fn resized_frame(bytes: &[u8], length: usize) -> Vec<u8> {
    let mut result = bytes[..length].to_vec();
    reframe(&mut result);
    result
}

#[allow(dead_code)]
fn prefix_frame(body: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 4];
    bytes.extend(body);
    reframe(&mut bytes);
    bytes
}

fn invalid_frame_lengths<T: VerCodable>(bytes: &[u8]) {
    let (header, start) = frame_layout(bytes);
    for missing in 0..4 {
        assert!(T::read_version(0, &bytes[..header + missing]).is_err());
    }
    let declared = u32::from_le_bytes(bytes[header..header + 4].try_into().unwrap());
    for length in [declared + 1, u32::MAX] {
        let mut bad = bytes.to_vec();
        bad[header..header + 4].copy_from_slice(&length.to_le_bytes());
        for version in [0, T::MAX_VERSION, u32::MAX] {
            assert!(T::read_version(version, &bad).is_err());
        }
    }
    if start == 0 {
        for length in 0u32..4 {
            let mut bad = bytes.to_vec();
            bad[..4].copy_from_slice(&length.to_le_bytes());
            assert!(T::read_version(0, &bad).is_err());
        }
    }
}

// Independent historical byte readers: no candidate or VerCodable calls.
#[allow(dead_code)]
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

#[allow(dead_code)]
impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn take(&mut self, len: usize) -> &'a [u8] {
        let end = self.pos.checked_add(len).unwrap();
        let result = &self.bytes[self.pos..end];
        self.pos = end;
        result
    }

    fn u8(&mut self) -> u8 {
        self.take(1)[0]
    }

    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.take(2).try_into().unwrap())
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take(4).try_into().unwrap())
    }

    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(self.take(8).try_into().unwrap())
    }

    fn string32(&mut self) -> String {
        let length = usize::try_from(self.u32()).unwrap();
        String::from_utf8(self.take(length).to_vec()).unwrap()
    }

    fn string64(&mut self) -> String {
        let length = usize::try_from(self.u64()).unwrap();
        String::from_utf8(self.take(length).to_vec()).unwrap()
    }

    fn remaining(&self) -> bool {
        self.pos < self.bytes.len()
    }
}

fn generic_composition<T: VerCodable + PartialEq + Debug + Clone + 'static>(
    values: &[T],
    version: u32,
) {
    let vector = values.to_vec();
    let bytes = encode(&vector, version);
    let mut stream = bytes.clone();
    stream.extend(0x1234_5678u32.to_le_bytes());
    assert_eq!(
        Vec::<T>::read_version(version, &stream),
        Ok((vector, bytes.len()))
    );
    assert_eq!(
        u32::read_version(version, &stream[bytes.len()..]),
        Ok((0x1234_5678, 4))
    );
    let tuple = (values[0].clone(), values[1].clone(), 0x1234_5678u32);
    let bytes = encode(&tuple, version);
    let mut stream = bytes.clone();
    stream.extend([0xa5; 7]);
    assert_eq!(
        <(T, T, u32)>::read_version(version, &stream),
        Ok((tuple, bytes.len()))
    );
}

#[derive(Debug, PartialEq, vercode::Vercode)]
struct DerivedParent {
    child: Custom,
    tail: u32,
    #[version(4)]
    later: u64,
}

fn derived_parent_composition(value: Custom) {
    for version in [Custom::MAX_VERSION, 4] {
        let parent = DerivedParent {
            child: value.clone(),
            tail: 0x1234_5678,
            later: if version >= 4 {
                0x9876_5432_1234_5678
            } else {
                0
            },
        };
        let bytes = encode(&parent, version);
        let mut stream = bytes.clone();
        stream.extend([0xa5; 7]);
        assert_eq!(
            DerivedParent::read_version(version, &stream),
            Ok((parent, bytes.len()))
        );
        // The macro owns its root frame; only the custom child's envelope grows.
        let child_length = value.size_version(version).unwrap();
        let mut future_child = bytes[4..4 + child_length].to_vec();
        future_child.extend([0xff, 0x80, 7, 9]);
        reframe(&mut future_child);
        let mut future_parent = bytes[..4].to_vec();
        future_parent.extend(&future_child);
        future_parent.extend(&bytes[4 + child_length..]);
        let total = future_parent.len();
        future_parent[..4].copy_from_slice(&(total as u32).to_le_bytes());
        future_parent.extend([0xa5; 7]);
        assert_eq!(
            DerivedParent::read_version(version, &future_parent),
            Ok((
                DerivedParent {
                    child: value.clone(),
                    tail: 0x1234_5678,
                    later: if version >= 4 {
                        0x9876_5432_1234_5678
                    } else {
                        0
                    },
                },
                total
            ))
        );
        let (older, used) =
            DerivedParent::read_version(Custom::MAX_VERSION, &future_parent).unwrap();
        assert_eq!(older.child, value);
        assert_eq!(older.tail, 0x1234_5678);
        assert_eq!(older.later, 0);
        assert_eq!(used, total);
    }
}

fn check_following_sibling<T: VerCodable + PartialEq + Debug>(
    bytes: &[u8],
    expected: T,
    version: u32,
) {
    let mut stream = bytes.to_vec();
    stream.extend(0x1234_5678u32.to_le_bytes());
    let (actual, used) = T::read_version(version, &stream).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(used, bytes.len());
    assert_eq!(
        u32::read_version(version, &stream[used..]),
        Ok((0x1234_5678, 4))
    );
}

fn malformed_smoke<T: VerCodable>() {
    let mut seed = 0x7d39_4b21u32;
    for len in 0..96 {
        for _ in 0..16 {
            let mut bytes = vec![0; len];
            for byte in &mut bytes {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                *byte = seed as u8;
            }
            // Err or a valid value is acceptable; unwinding is not.
            if let Ok((_, used)) = T::read_version(T::MAX_VERSION, &bytes) {
                assert!(used > 0 && used <= bytes.len());
            }
        }
    }
}
