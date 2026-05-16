//! Runtime Helpers and Handle Boundary (Phase 16.1).
//! 
//! These structures define the interface that the VM provides to 
//! JIT-compiled code for heap operations.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use crate::abi::{JitHelperResult, JIT_STATUS_SUCCESS, JIT_STATUS_ERROR_INVALID_HANDLE};

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
    /// (HeapHandle) -> JitHelperResult (value=len, status)
    StringLen,
    /// (HeapHandle, i64) -> JitHelperResult (value=handle, status)
    ArrayGet,
    /// (HeapHandle, i64) -> JitHelperResult (value=handle, status)
    StructGetField,
    /// (HeapHandle) -> JitHelperResult (value=tag, status)
    EnumTag,
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

// --- Test Scaffold ---

/// A temporary testing scaffold to map handles to strings.
/// NOT the final runtime model.
static TEST_HANDLE_TABLE: OnceLock<Mutex<HashMap<u64, String>>> = OnceLock::new();

pub fn get_test_handle_table() -> &'static Mutex<HashMap<u64, String>> {
    TEST_HANDLE_TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Helper implementation for String.len()
/// 
/// Validates the opaque handle against the test table.
pub extern "C" fn tmpr_jit_string_len(handle: u64) -> JitHelperResult {
    let table = get_test_handle_table().lock().unwrap();
    if let Some(s) = table.get(&handle) {
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
