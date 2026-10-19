//! Runtime Helpers and Handle Boundary (Phase 16.2).
//!
//! These structures define the interface that the VM provides to
//! JIT-compiled code for heap operations.

use crate::abi::{HeapHandle, JitHelperResult};
use std::collections::HashMap;

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
    GenericLen,
    /// (ctx: *mut JitRuntimeContext, HeapHandle) -> JitHelperResult (value=byte_count, status)
    StringSize,
    GenericSize,
    /// (ctx: *mut JitRuntimeContext, HeapHandle) -> JitHelperResult (value=len, status)
    ArrayLen,
    /// (ctx: *mut JitRuntimeContext, HeapHandle, i64) -> JitHelperResult (value=handle, status)
    ArrayGet,
    GenericGetI64,
    /// (ctx: *mut JitRuntimeContext, HeapHandle, u64) -> JitHelperResult (value=i64_bits, status)
    StructGet,
    /// (ctx: *mut JitRuntimeContext, HeapHandle) -> JitHelperResult (value=tag, status)
    EnumTag,
    /// (ctx: *mut JitRuntimeContext, HeapHandle, expected_tag: u64, payload_index: u64) -> JitHelperResult (value=i64_bits, status)
    EnumUnwrap,
    /// (ctx: *mut JitRuntimeContext) -> JitHelperResult (value=handle, status)
    AllocStringEmpty,
    /// (ctx: *mut JitRuntimeContext, HeapHandle, HeapHandle) -> JitHelperResult (value=handle, status)
    StringConcat,
    /// (ctx: *mut JitRuntimeContext, HeapHandle, value: i64) -> JitHelperResult (value=handle, status)
    ArrayPushI64,
}

impl JitHelperRegistry {
    pub fn new_with_helpers() -> Self {
        let mut helpers = HashMap::new();

        helpers.insert(
            "tmpr_jit_string_len",
            JitHelperInfo {
                name: "tmpr_jit_string_len",
                signature: JitHelperSignature::StringLen,
                ptr: tmpr_jit_string_len as *const (),
            },
        );

        helpers.insert(
            "tmpr_jit_generic_len",
            JitHelperInfo {
                name: "tmpr_jit_generic_len",
                signature: JitHelperSignature::GenericLen,
                ptr: tmpr_jit_generic_len as *const (),
            },
        );

        helpers.insert(
            "tmpr_jit_string_size",
            JitHelperInfo {
                name: "tmpr_jit_string_size",
                signature: JitHelperSignature::StringSize,
                ptr: tmpr_jit_string_size as *const (),
            },
        );

        helpers.insert(
            "tmpr_jit_array_len",
            JitHelperInfo {
                name: "tmpr_jit_array_len",
                signature: JitHelperSignature::ArrayLen,
                ptr: tmpr_jit_array_len as *const (),
            },
        );

        helpers.insert(
            "tmpr_jit_generic_size",
            JitHelperInfo {
                name: "tmpr_jit_generic_size",
                signature: JitHelperSignature::GenericSize,
                ptr: tmpr_jit_generic_size as *const (),
            },
        );

        helpers.insert(
            "tmpr_jit_array_get_i64",
            JitHelperInfo {
                name: "tmpr_jit_array_get_i64",
                signature: JitHelperSignature::ArrayGet,
                ptr: tmpr_jit_array_get_i64 as *const (),
            },
        );

        helpers.insert(
            "tmpr_jit_generic_get_i64",
            JitHelperInfo {
                name: "tmpr_jit_generic_get_i64",
                signature: JitHelperSignature::GenericGetI64,
                ptr: tmpr_jit_generic_get_i64 as *const (),
            },
        );

        helpers.insert(
            "tmpr_jit_struct_get_i64",
            JitHelperInfo {
                name: "tmpr_jit_struct_get_i64",
                signature: JitHelperSignature::StructGet,
                ptr: tmpr_jit_struct_get_i64 as *const (),
            },
        );

        helpers.insert(
            "tmpr_jit_enum_tag",
            JitHelperInfo {
                name: "tmpr_jit_enum_tag",
                signature: JitHelperSignature::EnumTag,
                ptr: tmpr_jit_enum_tag as *const (),
            },
        );

        helpers.insert(
            "tmpr_jit_enum_unwrap_i64",
            JitHelperInfo {
                name: "tmpr_jit_enum_unwrap_i64",
                signature: JitHelperSignature::EnumUnwrap,
                ptr: tmpr_jit_enum_unwrap_i64 as *const (),
            },
        );

        helpers.insert(
            "tmpr_jit_alloc_string_empty",
            JitHelperInfo {
                name: "tmpr_jit_alloc_string_empty",
                signature: JitHelperSignature::AllocStringEmpty,
                ptr: tmpr_jit_alloc_string_empty as *const (),
            },
        );

        helpers.insert(
            "tmpr_jit_string_concat",
            JitHelperInfo {
                name: "tmpr_jit_string_concat",
                signature: JitHelperSignature::StringConcat,
                ptr: tmpr_jit_string_concat as *const (),
            },
        );

        helpers.insert(
            "tmpr_jit_array_push_i64",
            JitHelperInfo {
                name: "tmpr_jit_array_push_i64",
                signature: JitHelperSignature::ArrayPushI64,
                ptr: tmpr_jit_array_push_i64 as *const (),
            },
        );

        Self { helpers }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum JitHandleKey {
    String(String),
    ArrayInt64(Vec<i64>),
    ArrayUInt8(Vec<u8>),
    ArrayBool(Vec<bool>),
}

/// A prototype runtime context for JIT heap operations.
pub struct JitRuntimeContext {
    heap_handles: HashMap<u64, JitHeapObject>,
    next_handle: u64,
    pub total_allocations: u64,
    pub helper_calls: u64,
    pub handle_cache: HashMap<JitHandleKey, HeapHandle>,
    pub cache_hits: u64,
    pub cache_misses: u64,
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
            total_allocations: 0,
            helper_calls: 0,
            handle_cache: HashMap::new(),
            cache_hits: 0,
            cache_misses: 0,
        }
    }

    pub fn object_count(&self) -> usize {
        self.heap_handles.len()
    }

    pub fn allocation_count(&self) -> u64 {
        self.total_allocations
    }

    pub fn alloc_string(&mut self, value: String) -> HeapHandle {
        let handle = self.next_handle;
        self.heap_handles
            .insert(handle, JitHeapObject::String(value));
        self.next_handle += 1;
        self.total_allocations += 1;
        HeapHandle(handle)
    }

    pub fn alloc_array(&mut self, values: Vec<JitHeapScalar>) -> HeapHandle {
        let handle = self.next_handle;
        self.heap_handles
            .insert(handle, JitHeapObject::Array(values));
        self.next_handle += 1;
        self.total_allocations += 1;
        HeapHandle(handle)
    }

    pub fn alloc_or_get_string(&mut self, value: &str) -> HeapHandle {
        let key = JitHandleKey::String(value.to_string());
        if let Some(&handle) = self.handle_cache.get(&key) {
            self.cache_hits += 1;
            handle
        } else {
            self.cache_misses += 1;
            let handle = self.alloc_string(value.to_string());
            self.handle_cache.insert(key, handle);
            handle
        }
    }

    pub fn alloc_or_get_array_int64(&mut self, values: &[i64]) -> HeapHandle {
        let key = JitHandleKey::ArrayInt64(values.to_vec());
        if let Some(&handle) = self.handle_cache.get(&key) {
            self.cache_hits += 1;
            handle
        } else {
            self.cache_misses += 1;
            let scalars = values.iter().map(|&i| JitHeapScalar::Int64(i)).collect();
            let handle = self.alloc_array(scalars);
            self.handle_cache.insert(key, handle);
            handle
        }
    }

    pub fn alloc_struct(
        &mut self,
        type_name: impl Into<String>,
        fields: Vec<(String, JitHeapScalar)>,
    ) -> HeapHandle {
        let handle = self.next_handle;
        self.heap_handles.insert(
            handle,
            JitHeapObject::Struct(JitHeapStruct {
                type_name: type_name.into(),
                fields,
            }),
        );
        self.next_handle += 1;
        self.total_allocations += 1;
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
        self.heap_handles.insert(
            handle,
            JitHeapObject::Enum(JitHeapEnum {
                type_name: type_name.into(),
                variant_name: variant_name.into(),
                tag,
                payloads,
            }),
        );
        self.next_handle += 1;
        self.total_allocations += 1;
        HeapHandle(handle)
    }

    pub fn get_string(&self, handle: HeapHandle) -> Result<&String, crate::abi::JitStatus> {
        if let Some(obj) = self.heap_handles.get(&handle.0) {
            match obj {
                JitHeapObject::String(s) => Ok(s),
                _ => Err(crate::abi::JIT_STATUS_ERROR_TYPE),
            }
        } else {
            Err(crate::abi::JIT_STATUS_ERROR_INVALID_HANDLE)
        }
    }

    pub fn string_concat(&mut self, left: HeapHandle, right: HeapHandle) -> JitHelperResult {
        let left_str = match self.get_string(left) {
            Ok(s) => s.to_owned(),
            Err(status) => return JitHelperResult { value: 0, status },
        };
        let right_str = match self.get_string(right) {
            Ok(s) => s.to_owned(),
            Err(status) => return JitHelperResult { value: 0, status },
        };

        let new_handle = self.alloc_string(format!("{}{}", left_str, right_str));
        JitHelperResult {
            value: new_handle.0,
            status: crate::abi::JIT_STATUS_SUCCESS,
        }
    }

    pub fn get_array(
        &self,
        handle: HeapHandle,
    ) -> Result<&Vec<JitHeapScalar>, crate::abi::JitStatus> {
        if let Some(obj) = self.heap_handles.get(&handle.0) {
            match obj {
                JitHeapObject::Array(arr) => Ok(arr),
                _ => Err(crate::abi::JIT_STATUS_ERROR_TYPE),
            }
        } else {
            Err(crate::abi::JIT_STATUS_ERROR_INVALID_HANDLE)
        }
    }

    pub fn array_push_i64(&mut self, array: HeapHandle, value: i64) -> JitHelperResult {
        let mut new_arr = match self.get_array(array) {
            Ok(arr) => {
                let mut clone = Vec::with_capacity(arr.len() + 1);
                // We cannot derive Clone for JitHeapScalar easily if HeapHandle doesn't,
                // but actually JitHeapScalar only has simple types for now. Let's do a manual clone to be safe.
                for item in arr {
                    clone.push(match item {
                        JitHeapScalar::Int64(v) => JitHeapScalar::Int64(*v),
                        JitHeapScalar::UInt8(v) => JitHeapScalar::UInt8(*v),
                        JitHeapScalar::Bool(v) => JitHeapScalar::Bool(*v),
                        JitHeapScalar::Handle(h) => {
                            JitHeapScalar::Handle(crate::abi::HeapHandle(h.0))
                        }
                    });
                }
                clone
            }
            Err(status) => return JitHelperResult { value: 0, status },
        };
        new_arr.push(JitHeapScalar::Int64(value));

        let new_handle = self.alloc_array(new_arr);
        JitHelperResult {
            value: new_handle.0,
            status: crate::abi::JIT_STATUS_SUCCESS,
        }
    }

    pub fn clear(&mut self) {
        self.heap_handles.clear();
        self.next_handle = 1000;
        self.total_allocations = 0;
        self.helper_calls = 0;
        self.handle_cache.clear();
        self.cache_hits = 0;
        self.cache_misses = 0;
    }
}

/// Internal utility to safely resolve a heap handle within a context.
fn with_handle<T>(
    ctx: *mut JitRuntimeContext,
    handle: u64,
    f: impl FnOnce(&JitHeapObject) -> Result<T, crate::abi::JitStatus>,
) -> JitHelperResult
where
    T: Into<u64>,
{
    use crate::abi::{
        JIT_STATUS_ERROR_INVALID_CONTEXT, JIT_STATUS_ERROR_INVALID_HANDLE, JIT_STATUS_SUCCESS,
    };

    if ctx.is_null() {
        return JitHelperResult {
            value: 0,
            status: JIT_STATUS_ERROR_INVALID_CONTEXT,
        };
    }

    // Increment helper calls on the context pointer
    unsafe {
        (*ctx).helper_calls += 1;
    }

    let ctx_ref = unsafe { &*ctx };
    if let Some(obj) = ctx_ref.heap_handles.get(&handle) {
        match f(obj) {
            Ok(val) => JitHelperResult {
                value: val.into(),
                status: JIT_STATUS_SUCCESS,
            },
            Err(status) => JitHelperResult { value: 0, status },
        }
    } else {
        JitHelperResult {
            value: 0,
            status: JIT_STATUS_ERROR_INVALID_HANDLE,
        }
    }
}

/// Helper implementation for generic Len (String or Array)
pub extern "C" fn tmpr_jit_generic_len(
    ctx: *mut JitRuntimeContext,
    handle: u64,
) -> JitHelperResult {
    with_handle(ctx, handle, |obj| match obj {
        JitHeapObject::String(s) => Ok(s.chars().count() as u64),
        JitHeapObject::Array(arr) => Ok(arr.len() as u64),
        _ => Err(crate::abi::JIT_STATUS_ERROR_TYPE),
    })
}

/// Helper implementation for String.len() (Unicode scalar count)
pub extern "C" fn tmpr_jit_string_len(ctx: *mut JitRuntimeContext, handle: u64) -> JitHelperResult {
    with_handle(ctx, handle, |obj| match obj {
        JitHeapObject::String(s) => Ok(s.chars().count() as u64),
        _ => Err(crate::abi::JIT_STATUS_ERROR_TYPE),
    })
}

/// Helper implementation for String.size() (UTF-8 byte count)
pub extern "C" fn tmpr_jit_string_size(
    ctx: *mut JitRuntimeContext,
    handle: u64,
) -> JitHelperResult {
    with_handle(ctx, handle, |obj| match obj {
        JitHeapObject::String(s) => Ok(s.len() as u64),
        _ => Err(crate::abi::JIT_STATUS_ERROR_TYPE),
    })
}

/// Helper implementation for generic Size (String only, TypeError for Array)
pub extern "C" fn tmpr_jit_generic_size(
    ctx: *mut JitRuntimeContext,
    handle: u64,
) -> JitHelperResult {
    with_handle(ctx, handle, |obj| match obj {
        JitHeapObject::String(s) => Ok(s.len() as u64),
        JitHeapObject::Array(_) => Err(crate::abi::JIT_STATUS_ERROR_TYPE),
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

/// Helper implementation for generic Array.get_i64 (TypeError for String)
pub extern "C" fn tmpr_jit_generic_get_i64(
    ctx: *mut JitRuntimeContext,
    handle: u64,
    index: i64,
) -> JitHelperResult {
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

/// Helper implementation for Array.get(Int64)
///
/// ABI: (ctx: *mut JitRuntimeContext, handle: u64, index: i64) -> JitHelperResult
pub extern "C" fn tmpr_jit_array_get_i64(
    ctx: *mut JitRuntimeContext,
    handle: u64,
    index: i64,
) -> JitHelperResult {
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
pub extern "C" fn tmpr_jit_struct_get_i64(
    ctx: *mut JitRuntimeContext,
    handle: u64,
    field_id: u64,
) -> JitHelperResult {
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

/// Helper implementation for Enum.unwrap_i64
///
/// ABI: (ctx: *mut JitRuntimeContext, handle: u64, expected_tag: u64, payload_index: u64) -> JitHelperResult
pub extern "C" fn tmpr_jit_enum_unwrap_i64(
    ctx: *mut JitRuntimeContext,
    handle: u64,
    expected_tag: u64,
    payload_index: u64,
) -> JitHelperResult {
    with_handle(ctx, handle, |obj| match obj {
        JitHeapObject::Enum(e) => {
            if e.tag != expected_tag {
                return Err(crate::abi::JIT_STATUS_ERROR_WRONG_VARIANT);
            }
            if payload_index >= e.payloads.len() as u64 {
                return Err(crate::abi::JIT_STATUS_ERROR_OUT_OF_BOUNDS);
            }
            match &e.payloads[payload_index as usize] {
                JitHeapScalar::Int64(val) => Ok(*val as u64),
                _ => Err(crate::abi::JIT_STATUS_ERROR_TYPE),
            }
        }
        _ => Err(crate::abi::JIT_STATUS_ERROR_TYPE),
    })
}

/// Helper implementation for allocating an empty string.
///
/// ABI: (ctx: *mut JitRuntimeContext) -> JitHelperResult
pub extern "C" fn tmpr_jit_alloc_string_empty(ctx: *mut JitRuntimeContext) -> JitHelperResult {
    if ctx.is_null() {
        return JitHelperResult {
            value: 0,
            status: crate::abi::JIT_STATUS_ERROR_INVALID_CONTEXT,
        };
    }
    let ctx = unsafe { &mut *ctx };
    ctx.helper_calls += 1;
    let handle = ctx.alloc_string(String::new());
    JitHelperResult {
        value: handle.0,
        status: crate::abi::JIT_STATUS_SUCCESS,
    }
}

/// Helper implementation for concatenating two strings.
///
/// ABI: (ctx: *mut JitRuntimeContext, left_handle: u64, right_handle: u64) -> JitHelperResult
pub extern "C" fn tmpr_jit_string_concat(
    ctx: *mut JitRuntimeContext,
    left_handle: u64,
    right_handle: u64,
) -> JitHelperResult {
    if ctx.is_null() {
        return JitHelperResult {
            value: 0,
            status: crate::abi::JIT_STATUS_ERROR_INVALID_CONTEXT,
        };
    }
    let ctx = unsafe { &mut *ctx };
    ctx.helper_calls += 1;
    ctx.string_concat(
        crate::abi::HeapHandle(left_handle),
        crate::abi::HeapHandle(right_handle),
    )
}

/// Helper implementation for array push with value semantics.
///
/// ABI: (ctx: *mut JitRuntimeContext, array_handle: u64, value: i64) -> JitHelperResult
pub extern "C" fn tmpr_jit_array_push_i64(
    ctx: *mut JitRuntimeContext,
    array_handle: u64,
    value: i64,
) -> JitHelperResult {
    if ctx.is_null() {
        return JitHelperResult {
            value: 0,
            status: crate::abi::JIT_STATUS_ERROR_INVALID_CONTEXT,
        };
    }
    let ctx = unsafe { &mut *ctx };
    ctx.helper_calls += 1;
    ctx.array_push_i64(crate::abi::HeapHandle(array_handle), value)
}
