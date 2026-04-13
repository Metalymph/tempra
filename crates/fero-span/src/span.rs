pub type ByteOffset = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: ByteOffset,
    pub end: ByteOffset,
}

impl Span {
    pub fn new(start: ByteOffset, end: ByteOffset) -> Self {
        Self { start, end }
    }
}
