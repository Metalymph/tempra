//! Design-only skeleton for JIT Runtime Helpers (Phase 16.0).
//! 
//! These structures define the interface that the VM will provide to 
//! JIT-compiled code for heap operations.

// use crate::abi::{HeapHandle, JitHelperResult}; // Will be used in 16.1+

/// Registry of available runtime helpers.
/// 
/// In Phase 16.1+, this will be used to map helper names to 
/// function pointers during Cranelift lowering.
pub struct JitHelperRegistry {
    pub helpers: Vec<JitHelperInfo>,
}

pub struct JitHelperInfo {
    pub name: &'static str,
    pub signature: JitHelperSignature,
    // pub ptr: *const (), // Added in 16.1
}

#[derive(Debug, Clone)]
pub enum JitHelperSignature {
    /// (HeapHandle) -> i64
    StringLen,
    /// (HeapHandle, i64) -> (HeapHandle, i64)
    ArrayGet,
    /// (HeapHandle, i64) -> (HeapHandle, i64)
    StructGetField,
    /// (HeapHandle) -> i64 (tag_id)
    EnumTag,
}

impl JitHelperRegistry {
    pub fn new_stubs() -> Self {
        Self {
            helpers: vec![
                JitHelperInfo {
                    name: "tmpr_jit_string_len",
                    signature: JitHelperSignature::StringLen,
                },
                JitHelperInfo {
                    name: "tmpr_jit_array_get",
                    signature: JitHelperSignature::ArrayGet,
                },
            ],
        }
    }
}

// Example of what a helper implementation will look like in tempra-vm:
/*
pub extern "C" fn tmpr_jit_string_len(handle: u64) -> i64 {
    // 1. Resolve handle to VmValue
    // 2. Validate it's a string
    // 3. Return length
    0
}
*/
