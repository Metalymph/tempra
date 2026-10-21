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
pub const JIT_STATUS_ERROR_WRONG_VARIANT: JitStatus = 8;
/// Allocation failed (OOM or internal runtime error).
pub const JIT_STATUS_ERROR_ALLOCATION_FAILED: JitStatus = 9;
/// A checked `Int64` operation overflowed.
pub const JIT_STATUS_ERROR_INT64_OVERFLOW: JitStatus = 10;
/// An `Int64` division used a zero divisor.
pub const JIT_STATUS_ERROR_INT64_DIVISION_BY_ZERO: JitStatus = 11;

/// A representation of a value passed across the JIT/VM boundary.
///
/// Note: HeapHandle(0) is reserved as the invalid/null handle.
#[derive(Debug, Clone, Copy)]
pub enum JitValueAbi {
    /// A 64-bit primitive (Int64 or internal Bool).
    Primitive(i64),
    /// A handle to a heap-allocated object.
    Handle(HeapHandle),
}

/// A representation of a value passed across the JIT/VM boundary.
///
/// # Boundary convention
///
/// Every fallible boundary function writes this value through a caller-provided
/// `*mut JitHelperResult` passed as the *first* argument and returns unit:
///
/// ```text
/// extern "C" fn(out: *mut JitHelperResult, <parameters...>) -> ()
/// ```
///
/// Returning the pair by value is not portable: SysV returns a 16-byte
/// aggregate in registers, while Windows x64 returns it through a hidden
/// pointer that displaces the first parameter. Passing storage explicitly keeps
/// the generated code and the Rust callee in agreement on every target.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct JitHelperResult {
    /// The returned value (e.g. handle or length).
    pub value: u64,
    /// The status of the operation (0 = OK).
    pub status: JitStatus,
}

/// Byte size of the out-parameter storage the JIT must provide for a
/// [`JitHelperResult`].
pub const JIT_RESULT_SIZE: u32 = 16;

/// Byte offset of [`JitHelperResult::value`] inside that storage.
pub const JIT_RESULT_VALUE_OFFSET: i32 = 0;

/// Byte offset of [`JitHelperResult::status`] inside that storage.
pub const JIT_RESULT_STATUS_OFFSET: i32 = 8;

const _: () = assert!(
    std::mem::size_of::<JitHelperResult>() == JIT_RESULT_SIZE as usize,
    "the JitHelperResult layout is part of the JIT boundary ABI"
);

impl JitHelperResult {
    /// Converts a raw JIT helper result into an idiomatic Rust Result.
    pub fn to_result(self) -> Result<u64, crate::error::JitRuntimeError> {
        if let Some(err) = crate::error::JitRuntimeError::from_status(self.status) {
            Err(err)
        } else {
            Ok(self.value)
        }
    }

    /// Converts a raw JIT helper result into an i64 Result (bit-preserving).
    pub fn to_i64_result(self) -> Result<i64, crate::error::JitRuntimeError> {
        self.to_result().map(|v| v as i64)
    }
}
