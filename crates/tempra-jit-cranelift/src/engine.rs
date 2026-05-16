use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use tempra_bytecode::bytecode::BytecodeModule;
use tempra_bytecode::function::{FunctionId, BytecodeType};
use crate::error::JitRuntimeError;

pub struct JitEngine {
    pub(crate) builder_context: FunctionBuilderContext,
    pub(crate) ctx: codegen::Context,
    pub(crate) module: JITModule,
}

impl JitEngine {
    pub fn new() -> Self {
        let mut flag_builder = settings::builder();
        // On AArch64, we often need to disable PIC/PLT for simple JIT usage
        // or ensure the ISA is correctly configured.
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        
        let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });
        let isa = isa_builder.finish(settings::Flags::new(flag_builder)).unwrap();
        
        let builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        let module = JITModule::new(builder);
        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: codegen::Context::new(),
            module,
        }
    }
}

#[derive(Clone, Copy)]
pub struct JitFunctionI64 {
    ptr: *const u8,
    arity: usize,
}

impl JitFunctionI64 {
    pub fn new(ptr: *const u8, arity: usize) -> Self {
        Self { ptr, arity }
    }

    pub fn call_i64(&self, args: &[i64]) -> Result<i64, JitRuntimeError> {
        if args.len() != self.arity {
            return Err(JitRuntimeError::Message(format!("Wrong arity: expected {}, got {}", self.arity, args.len())));
        }

        unsafe {
            match self.arity {
                0 => {
                    let func: fn() -> i64 = std::mem::transmute(self.ptr);
                    Ok(func())
                }
                1 => {
                    let func: fn(i64) -> i64 = std::mem::transmute(self.ptr);
                    Ok(func(args[0]))
                }
                2 => {
                    let func: fn(i64, i64) -> i64 = std::mem::transmute(self.ptr);
                    Ok(func(args[0], args[1]))
                }
                _ => Err(JitRuntimeError::Message(format!("Arity {} not yet supported by JIT caller", self.arity))),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum JitEligibility {
    Eligible,
    UnsupportedSignature(String),
    UnsupportedInstr { offset: usize, instr: String },
    UnsupportedType(String),
}

impl std::fmt::Display for JitEligibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Eligible => write!(f, "Eligible"),
            Self::UnsupportedSignature(s) => write!(f, "Unsupported signature: {}", s),
            Self::UnsupportedInstr { offset, instr } => write!(f, "Unsupported instruction '{}' at offset {}", instr, offset),
            Self::UnsupportedType(t) => write!(f, "Unsupported type: {}", t),
        }
    }
}

pub fn explain_jit_eligibility(module: &BytecodeModule, function_id: FunctionId) -> JitEligibility {
    let mut visited = std::collections::HashSet::new();
    explain_jit_eligibility_internal(module, function_id, &mut visited)
}

fn explain_jit_eligibility_internal(
    module: &BytecodeModule, 
    function_id: FunctionId,
    visited: &mut std::collections::HashSet<FunctionId>
) -> JitEligibility {
    if visited.contains(&function_id) {
        return JitEligibility::UnsupportedInstr { offset: 0, instr: "Call target not JIT eligible (recursion)".into() };
    }
    visited.insert(function_id);

    let func = match module.functions.get(function_id.0 as usize) {
        Some(f) => f,
        None => {
            visited.remove(&function_id);
            return JitEligibility::UnsupportedSignature("Function not found".into());
        }
    };

    // Params must be Int64
    for (i, p) in func.params.iter().enumerate() {
        if *p != BytecodeType::Int64 {
            visited.remove(&function_id);
            return JitEligibility::UnsupportedType(format!("Parameter {} has unsupported type {:?}", i, p));
        }
    }
    // Return must be Int64
    if func.ret != BytecodeType::Int64 {
        visited.remove(&function_id);
        return JitEligibility::UnsupportedType(format!("Return has unsupported type {:?}", func.ret));
    }
    // Locals must be Int64 or Bool (for comparisons)
    for (i, l) in func.locals.iter().enumerate() {
        if *l != BytecodeType::Int64 && *l != BytecodeType::Bool {
            visited.remove(&function_id);
            return JitEligibility::UnsupportedType(format!("Local {} has unsupported type {:?}", i, l));
        }
    }

    // Instructions must be supported
    for (offset, instr) in func.code.iter().enumerate() {
        if let tempra_bytecode::opcode::BytecodeInstr::Call(target_id, _) = instr {
            let target_eligibility = explain_jit_eligibility_internal(module, *target_id, visited);
            if target_eligibility != JitEligibility::Eligible {
                visited.remove(&function_id);
                if let JitEligibility::UnsupportedInstr { instr: ref target_instr, .. } = target_eligibility {
                    if target_instr.contains("recursion") {
                        return JitEligibility::UnsupportedInstr { offset, instr: "Call target not JIT eligible (recursion)".into() };
                    }
                }
                return JitEligibility::UnsupportedInstr { offset, instr: format!("Call target not JIT eligible: {}", target_eligibility) };
            }
        } else if !crate::lower::is_instr_supported(instr) {
            visited.remove(&function_id);
            return JitEligibility::UnsupportedInstr { offset, instr: format!("{:?}", instr) };
        }
    }

    visited.remove(&function_id);
    JitEligibility::Eligible
}

pub fn can_jit_function(module: &BytecodeModule, function_id: FunctionId) -> bool {
    matches!(explain_jit_eligibility(module, function_id), JitEligibility::Eligible)
}
