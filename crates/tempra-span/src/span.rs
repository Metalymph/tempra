//! Byte-offset based spans for source tracking.

/// A 0-indexed byte offset into a source file.
pub type ByteOffset = u32;

/// A range in the source code.
///
/// Spans are inclusive of `start` and exclusive of `end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// Start offset in bytes.
    pub start: ByteOffset,
    /// End offset in bytes.
    pub end: ByteOffset,
}

impl Span {
    pub fn new(start: ByteOffset, end: ByteOffset) -> Self {
        Self { start, end }
    }

    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}
