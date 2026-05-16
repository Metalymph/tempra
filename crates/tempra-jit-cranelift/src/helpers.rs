//! Runtime Helpers and Handle Boundary (Phase 16.2).
//! 
//! These structures define the interface that the VM provides to 
//! JIT-compiled code for heap operations.

use std::collections::HashMap;
use crate::abi::{HeapHandle, JitHelperResult};

/// Registry of available runtime helpers.
pub struct JitHelperRegistry {
    pub helpers: HashMap<&'static str, JitHelperInfo>,
}

pub struct JitHelperInfo {
    pub name: &'static str,
    pub signature: JitHelperSignature,
    pub ptr: *const (),
}

#[derive(Debug, Clone)]
pub enum JitHelperSignature {
    /// (ctx: *mut JitRuntimeContext, HeapHandle) -> JitHelperResult (value=len, status)
    StringLen,
    /// (ctx: *mut JitRuntimeContext, HeapHandle) -> JitHelperResult (value=byte_count, status)
    StringSize,
    /// (ctx: *mut JitRuntimeContext, HeapHandle) -> JitHelperResult (value=len, status)
    ArrayLen,
    /// (ctx: *mut JitRuntimeContext, HeapHandle, i64) -> JitHelperResult (value=handle, status)
    ArrayGet,
}

impl JitHelperRegistry {
    pub fn new_with_helpers() -> Self {
        let mut helpers = HashMap::new();
        
        helpers.insert("tmpr_jit_string_len", JitHelperInfo {
            name: "tmpr_jit_string_len",
            signature: JitHelperSignature::StringLen,
            ptr: tmpr_jit_string_len as *const (),
        });

        helpers.insert("tmpr_jit_string_size", JitHelperInfo {
            name: "tmpr_jit_string_size",
            signature: JitHelperSignature::StringSize,
            ptr: tmpr_jit_string_size as *const (),
        });

        helpers.insert("tmpr_jit_array_len", JitHelperInfo {
            name: "tmpr_jit_array_len",
            signature: JitHelperSignature::ArrayLen,
            ptr: tmpr_jit_array_len as *const (),
        });

        Self { helpers }
    }
}

/// A prototype runtime context for JIT heap operations.
pub struct JitRuntimeContext {
    heap_handles: HashMap<u64, JitHeapObject>,
    next_handle: u64,
}

/// Prototype heap objects visible to JIT.
pub enum JitHeapObject {
    String(String),
    Array(Vec<JitHeapScalar>),
}

/// Prototype scalar values within heap objects.
pub enum JitHeapScalar {
    Int64(i64),
    UInt8(u8),
    Bool(bool),
    Handle(HeapHandle),
}

impl JitRuntimeContext {
    pub fn new() -> Self {
        Self {
            heap_handles: HashMap::new(),
            next_handle: 1000,
        }
    }

    pub fn alloc_string(&mut self, value: String) -> HeapHandle {
        let handle = self.next_handle;
        self.heap_handles.insert(handle, JitHeapObject::String(value));
        self.next_handle += 1;
        HeapHandle(handle)
    }

    pub fn alloc_array(&mut self, values: Vec<JitHeapScalar>) -> HeapHandle {
        let handle = self.next_handle;
        self.heap_handles.insert(handle, JitHeapObject::Array(values));
        self.next_handle += 1;
        HeapHandle(handle)
    }

    pub fn clear(&mut self) {
        self.heap_handles.clear();
        self.next_handle = 1000;
    }
}

/// Internal utility to safely resolve a heap handle within a context.
fn with_handle<T>(
    ctx: *mut JitRuntimeContext,
    handle: u64,
    f: impl FnOnce(&JitHeapObject) -> Result<T, crate::abi::JitStatus>,
) -> JitHelperResult where T: Into<u64> {
    use crate::abi::{JIT_STATUS_SUCCESS, JIT_STATUS_ERROR_INVALID_HANDLE, JIT_STATUS_ERROR_INVALID_CONTEXT};
    
    if ctx.is_null() {
        return JitHelperResult {
            value: 0,
            status: JIT_STATUS_ERROR_INVALID_CONTEXT,
        };
    }

    let ctx_ref = unsafe { &*ctx };
    if let Some(obj) = ctx_ref.heap_handles.get(&handle) {
        match f(obj) {
            Ok(val) => JitHelperResult {
                value: val.into(),
                status: JIT_STATUS_SUCCESS,
            },
            Err(status) => JitHelperResult {
                value: 0,
                status,
            },
        }
    } else {
        JitHelperResult {
            value: 0,
            status: JIT_STATUS_ERROR_INVALID_HANDLE,
        }
    }
}

/// Helper implementation for String.len() (Unicode scalar count)
pub extern "C" fn tmpr_jit_string_len(ctx: *mut JitRuntimeContext, handle: u64) -> JitHelperResult {
    with_handle(ctx, handle, |obj| match obj {
        JitHeapObject::String(s) => Ok(s.chars().count() as u64),
        _ => Err(crate::abi::JIT_STATUS_ERROR_TYPE),
    })
}

/// Helper implementation for String.size() (UTF-8 byte count)
pub extern "C" fn tmpr_jit_string_size(ctx: *mut JitRuntimeContext, handle: u64) -> JitHelperResult {
    with_handle(ctx, handle, |obj| match obj {
        JitHeapObject::String(s) => Ok(s.len() as u64),
        _ => Err(crate::abi::JIT_STATUS_ERROR_TYPE),
    })
}

/// Helper implementation for Array.len()
pub extern "C" fn tmpr_jit_array_len(ctx: *mut JitRuntimeContext, handle: u64) -> JitHelperResult {
    with_handle(ctx, handle, |obj| match obj {
        JitHeapObject::Array(arr) => Ok(arr.len() as u64),
        _ => Err(crate::abi::JIT_STATUS_ERROR_TYPE),
    })
}
