use std::num::NonZeroU32;

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Point {
    pub x: u16,
    pub y: u16,
    /// Wire version 1.
    pub confidence: u8,
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Sample {
    pub id: u64,
    pub point: Point,
    pub tail: u32,
    pub flags: Option<bool>,
    pub ticket: Option<NonZeroU32>,
    pub bytes: [u8; 8],
    pub samples: Vec<u16>,
    pub entries: Vec<Entry>,
    /// Wire version 2. All other Sample fields are version 0.
    pub label: String,
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Entry {
    pub point: Point,
    pub weight: u32,
}
