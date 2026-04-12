/// A half-open byte range `[lo, hi)` into a source file.
///
/// Spans are the primary mechanism for mapping compiler artifacts back to
/// user-written code. They are intentionally `Copy` so they can be threaded
/// through every layer of the pipeline at negligible cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// Inclusive start offset (bytes from the beginning of the source).
    pub lo: usize,
    /// Exclusive end offset.
    pub hi: usize,
}

impl Span {
    /// Creates a span covering `[lo, hi)`.
    pub fn new(lo: usize, hi: usize) -> Self {
        debug_assert!(lo <= hi, "span lo ({lo}) must be <= hi ({hi})");
        Self { lo, hi }
    }

    /// A zero-length span used when no real location is available.
    pub fn dummy() -> Self {
        Self { lo: 0, hi: 0 }
    }

    /// Returns the byte length of this span.
    pub fn len(&self) -> usize {
        self.hi - self.lo
    }

    /// Returns `true` if the span covers zero bytes.
    pub fn is_empty(&self) -> bool {
        self.lo == self.hi
    }
}
