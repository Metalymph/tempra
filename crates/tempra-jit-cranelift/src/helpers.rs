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
    /// (ctx: *mut JitRuntimeContext, HeapHandle, u64) -> JitHelperResult (value=i64_bits, status)
    StructGet,
    /// (ctx: *mut JitRuntimeContext, HeapHandle) -> JitHelperResult (value=tag, status)
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

        helpers.insert("tmpr_jit_array_get_i64", JitHelperInfo {
            name: "tmpr_jit_array_get_i64",
            signature: JitHelperSignature::ArrayGet,
            ptr: tmpr_jit_array_get_i64 as *const (),
        });

        helpers.insert("tmpr_jit_struct_get_i64", JitHelperInfo {
            name: "tmpr_jit_struct_get_i64",
            signature: JitHelperSignature::StructGet,
            ptr: tmpr_jit_struct_get_i64 as *const (),
        });

        helpers.insert("tmpr_jit_enum_tag", JitHelperInfo {
            name: "tmpr_jit_enum_tag",
            signature: JitHelperSignature::EnumTag,
            ptr: tmpr_jit_enum_tag as *const (),
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
    Struct(JitHeapStruct),
    Enum(JitHeapEnum),
}

/// Prototype struct object.
pub struct JitHeapStruct {
    pub type_name: String,
    pub fields: Vec<(String, JitHeapScalar)>,
}

/// Prototype enum object.
pub struct JitHeapEnum {
    pub type_name: String,
    pub variant_name: String,
    pub tag: u64,
    pub payloads: Vec<JitHeapScalar>,
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

    pub fn alloc_struct(&mut self, type_name: impl Into<String>, fields: Vec<(String, JitHeapScalar)>) -> HeapHandle {
        let handle = self.next_handle;
        self.heap_handles.insert(handle, JitHeapObject::Struct(JitHeapStruct {
            type_name: type_name.into(),
            fields,
        }));
        self.next_handle += 1;
        HeapHandle(handle)
    }

    pub fn alloc_enum(
        &mut self,
        type_name: impl Into<String>,
        variant_name: impl Into<String>,
        tag: u64,
        payloads: Vec<JitHeapScalar>,
    ) -> HeapHandle {
        let handle = self.next_handle;
        self.heap_handles.insert(handle, JitHeapObject::Enum(JitHeapEnum {
            type_name: type_name.into(),
            variant_name: variant_name.into(),
            tag,
            payloads,
        }));
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

/// Helper implementation for Array.get(Int64)
/// 
/// ABI: (ctx: *mut JitRuntimeContext, handle: u64, index: i64) -> JitHelperResult
pub extern "C" fn tmpr_jit_array_get_i64(ctx: *mut JitRuntimeContext, handle: u64, index: i64) -> JitHelperResult {
    with_handle(ctx, handle, |obj| match obj {
        JitHeapObject::Array(arr) => {
            if index < 0 || index as usize >= arr.len() {
                return Err(crate::abi::JIT_STATUS_ERROR_OUT_OF_BOUNDS);
            }
            match &arr[index as usize] {
                JitHeapScalar::Int64(val) => Ok(*val as u64),
                _ => Err(crate::abi::JIT_STATUS_ERROR_TYPE),
            }
        }
        _ => Err(crate::abi::JIT_STATUS_ERROR_TYPE),
    })
}

/// Helper implementation for Struct.get(field_id)
/// 
/// field_id is prototype-only and index-based.
/// Production will need stable field IDs from compiler metadata.
/// 
/// ABI: (ctx: *mut JitRuntimeContext, handle: u64, field_id: u64) -> JitHelperResult
pub extern "C" fn tmpr_jit_struct_get_i64(ctx: *mut JitRuntimeContext, handle: u64, field_id: u64) -> JitHelperResult {
    with_handle(ctx, handle, |obj| match obj {
        JitHeapObject::Struct(s) => {
            if field_id >= s.fields.len() as u64 {
                return Err(crate::abi::JIT_STATUS_ERROR_OUT_OF_BOUNDS);
            }
            match &s.fields[field_id as usize].1 {
                JitHeapScalar::Int64(val) => Ok(*val as u64),
                _ => Err(crate::abi::JIT_STATUS_ERROR_TYPE),
            }
        }
        _ => Err(crate::abi::JIT_STATUS_ERROR_TYPE),
    })
}

/// Helper implementation for Enum.tag
/// 
/// ABI: (ctx: *mut JitRuntimeContext, handle: u64) -> JitHelperResult
pub extern "C" fn tmpr_jit_enum_tag(ctx: *mut JitRuntimeContext, handle: u64) -> JitHelperResult {
    with_handle(ctx, handle, |obj| match obj {
        JitHeapObject::Enum(e) => Ok(e.tag),
        _ => Err(crate::abi::JIT_STATUS_ERROR_TYPE),
    })
}
