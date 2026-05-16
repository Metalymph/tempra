//! Runtime Helpers and Handle Boundary (Phase 16.2).
//! 
//! These structures define the interface that the VM provides to 
//! JIT-compiled code for heap operations.

use std::collections::HashMap;
use crate::abi::{HeapHandle, JitHelperResult, JIT_STATUS_SUCCESS, JIT_STATUS_ERROR_INVALID_HANDLE};

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

        Self { helpers }
    }
}

/// A prototype runtime context for JIT heap operations.
/// 
/// Responsibility:
/// - Manages a local "arena" of heap values visible to JIT code.
/// - Validates handles during helper calls.
/// - This is a prototype context; the final version will be VM-owned.
pub struct JitRuntimeContext {
    heap_handles: HashMap<u64, String>,
    next_handle: u64,
}

impl JitRuntimeContext {
    pub fn new() -> Self {
        Self {
            heap_handles: HashMap::new(),
            next_handle: 1000, // Start handles at 1000 to avoid collision with small constants
        }
    }

    pub fn alloc_string(&mut self, value: String) -> HeapHandle {
        let handle = self.next_handle;
        self.heap_handles.insert(handle, value);
        self.next_handle += 1;
        HeapHandle(handle)
    }

    pub fn clear(&mut self) {
        self.heap_handles.clear();
        self.next_handle = 1000;
    }
}

/// Helper implementation for String.len()
/// 
/// ABI: (ctx: *mut JitRuntimeContext, handle: u64) -> JitHelperResult
pub extern "C" fn tmpr_jit_string_len(ctx: *mut JitRuntimeContext, handle: u64) -> JitHelperResult {
    if ctx.is_null() {
        return JitHelperResult {
            value: 0,
            status: JIT_STATUS_ERROR_INVALID_HANDLE,
        };
    }

    let ctx_ref = unsafe { &*ctx };
    if let Some(s) = ctx_ref.heap_handles.get(&handle) {
        JitHelperResult {
            value: s.chars().count() as u64,
            status: JIT_STATUS_SUCCESS,
        }
    } else {
        JitHelperResult {
            value: 0,
            status: JIT_STATUS_ERROR_INVALID_HANDLE,
        }
    }
}
