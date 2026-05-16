//! JIT ABI and Value representation for Phase 16.0+.

/// An opaque token representing a heap-allocated value in the VM.
/// 
/// The JIT and native code must treat this as an opaque u64.
/// It must NOT be dereferenced directly from native code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct HeapHandle(pub u64);

/// The status code returned by fallible JIT helpers.
/// 
/// 0 means Success.
/// Non-zero values represent specific runtime errors.
pub type JitStatus = i64;

pub const JIT_STATUS_SUCCESS: JitStatus = 0;
pub const JIT_STATUS_ERROR_BOUNDS: JitStatus = 1;
pub const JIT_STATUS_ERROR_TYPE: JitStatus = 2;
pub const JIT_STATUS_ERROR_NULL: JitStatus = 3;
pub const JIT_STATUS_ERROR_INVALID_HANDLE: JitStatus = 4;
pub const JIT_STATUS_ERROR_MISSING_HELPER: JitStatus = 5;
pub const JIT_STATUS_ERROR_INVALID_CONTEXT: JitStatus = 6;
pub const JIT_STATUS_ERROR_OUT_OF_BOUNDS: JitStatus = 7;

/// A representation of a value passed across the JIT/VM boundary.
#[derive(Debug, Clone, Copy)]
pub enum JitValueAbi {
    /// A 64-bit primitive (Int64 or internal Bool).
    Primitive(i64),
    /// A handle to a heap-allocated object.
    Handle(HeapHandle),
}

/// The result of a fallible JIT helper call.
#[repr(C)]
pub struct JitHelperResult {
    /// The primary result value (e.g. a handle or a primitive).
    pub value: u64,
    /// The status of the operation.
    pub status: JitStatus,
}
