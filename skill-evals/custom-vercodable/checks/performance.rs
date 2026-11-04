#![allow(dead_code)]

// The macro model and conversions are independent of candidate implementation.
include!("../tests/skill_eval_contract.rs");

use std::hint::black_box;
use std::time::Instant;

#[derive(Clone, Copy)]
enum Method {
    Write,
    Read,
    Size,
}

fn measure<T: VerCodable>(
    value: &T,
    version: u32,
    bytes: &[u8],
    output: &mut [u8],
    iterations: usize,
    method: Method,
) -> f64 {
    let start = Instant::now();
    match method {
        Method::Write => {
            for _ in 0..iterations {
                black_box(
                    black_box(value)
                        .write_version(black_box(version), black_box(&mut *output))
                        .unwrap(),
                );
            }
        }
        Method::Read => {
            for _ in 0..iterations {
                black_box(T::read_version(black_box(version), black_box(bytes)).unwrap());
            }
        }
        Method::Size => {
            for _ in 0..iterations {
                black_box(black_box(value).size_version(black_box(version)).unwrap());
            }
        }
    }
    start.elapsed().as_nanos() as f64 / iterations as f64
}

fn main() {
    for elements in [32, 512, 4096] {
        let mut value = sample();
        value.samples = (0..64).map(|index| (index * 19) as u16).collect();
        value.entries = (0..elements)
            .map(|index| Entry {
                point: Point {
                    x: (index * 7) as u16,
                    y: (index * 13 + 1) as u16,
                    confidence: (index % 251) as u8,
                },
                weight: (index * 37 + 17) as u32,
            })
            .collect();
        for version in [0, 2] {
            let reference = reference(&value);
            let custom_bytes = encode(&value, version);
            let derive_bytes = encode(&reference, version);
            let expected = projected(&value, version);
            // Different wire formats: compare decoded VALUES, never their bytes.
            assert_eq!(
                Sample::read_version(version, &custom_bytes),
                Ok((expected.clone(), custom_bytes.len()))
            );
            assert_eq!(
                RefSample::read_version(version, &derive_bytes),
                Ok((crate::reference(&expected), derive_bytes.len()))
            );
            assert!(custom_bytes.len() < derive_bytes.len());
            assert_eq!(derive_bytes.len() - custom_bytes.len(), 4 + 8 * elements);
            assert_eq!(
                u32::from_le_bytes(custom_bytes[..4].try_into().unwrap()) as usize,
                custom_bytes.len()
            );

            let mut custom_output = vec![0; custom_bytes.len()];
            let mut derive_output = vec![0; derive_bytes.len()];
            let mut custom = [[0.0; 9]; 3];
            let mut derived = [[0.0; 9]; 3];
            let traversal_iterations = match elements {
                32 => 20_000,
                512 => 2_000,
                _ => 300,
            };
            let size_iterations = 1_000_000;
            for (index, method) in [Method::Write, Method::Read, Method::Size]
                .into_iter()
                .enumerate()
            {
                let iterations = match method {
                    Method::Size => size_iterations,
                    _ => traversal_iterations,
                };
                // Warm both paths; allocations and input/output setup are not timed.
                measure(
                    &value,
                    version,
                    &custom_bytes,
                    &mut custom_output,
                    32,
                    method,
                );
                measure(
                    &reference,
                    version,
                    &derive_bytes,
                    &mut derive_output,
                    32,
                    method,
                );
                for sample in 0..9 {
                    let mut custom_once = || {
                        measure(
                            &value,
                            version,
                            &custom_bytes,
                            &mut custom_output,
                            iterations,
                            method,
                        )
                    };
                    let mut derive_once = || {
                        measure(
                            &reference,
                            version,
                            &derive_bytes,
                            &mut derive_output,
                            iterations,
                            method,
                        )
                    };
                    if sample % 2 == 0 {
                        custom[index][sample] = custom_once();
                        derived[index][sample] = derive_once();
                    } else {
                        derived[index][sample] = derive_once();
                        custom[index][sample] = custom_once();
                    }
                }
            }
            println!(
                "{{\"elements\":{elements},\"version\":{version},\
                 \"iterations_per_method\":{{\"write\":{traversal_iterations},\
                 \"read_owned\":{traversal_iterations},\"size\":{size_iterations}}},\
                 \"samples\":9,\"custom_bytes\":{},\"derive_bytes\":{},\"savings_bytes\":{},\
                 \"custom_ns\":{{\"write\":{:?},\"read_owned\":{:?},\"size\":{:?}}},\
                 \"derive_ns\":{{\"write\":{:?},\"read_owned\":{:?},\"size\":{:?}}}}}",
                custom_bytes.len(),
                derive_bytes.len(),
                derive_bytes.len() - custom_bytes.len(),
                custom[0],
                custom[1],
                custom[2],
                derived[0],
                derived[1],
                derived[2],
            );
        }
    }
}
