#[derive(Debug, Default, PartialEq, Clone)]
pub enum Mode {
    #[default]
    Unknown,
    Idle,
    Pair(u16, u32),
    Named {
        code: u32,
        /// Wire version 1.
        extra: u16,
    },
    /// Tag stays in base; payload is deferred to global pass 2.
    /// Earlier writes reject this variant; earlier reads use Unknown.
    Added([u8; 3]),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Strict {
    Idle,
    Value(u16),
}

#[derive(
    Debug,
    Default,
    PartialEq,
    Clone,
    Copy,
    zerocopy::IntoBytes,
    zerocopy::FromBytes,
    zerocopy::KnownLayout,
    zerocopy::Immutable,
)]
#[repr(C)]
pub struct Block {
    pub a: u32,
    pub b: u32,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Frame {
    pub mode: Mode,
    pub strict: Strict,
    pub tail: u32,
    pub block: Block,
    pub code: Code,
    pub modes: Vec<Mode>,
    /// Global pass 2. Its entire new shape starts here, including Mode::Named.extra.
    pub extension: Option<Extension>,
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
#[repr(u8)]
pub enum Code {
    #[default]
    Unknown = 0,
    Ready = 7,
    Busy = 13,
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Extension {
    pub modes: Vec<Mode>,
    /// Global pass 3; no repeated option tag, vector count, or mode tags.
    pub checksum: u32,
}
