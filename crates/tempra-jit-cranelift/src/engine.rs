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

pub fn can_jit_function(module: &BytecodeModule, function_id: FunctionId) -> bool {
    let func = match module.functions.get(function_id.0 as usize) {
        Some(f) => f,
        None => return false,
    };

    // Params must be Int64
    if !func.params.iter().all(|p| *p == BytecodeType::Int64) {
        return false;
    }
    // Return must be Int64
    if func.ret != BytecodeType::Int64 {
        return false;
    }
    // Locals must be Int64
    if !func.locals.iter().all(|l| *l == BytecodeType::Int64) {
        return false;
    }

    // Instructions must be supported
    for instr in &func.code {
        if !crate::lower::is_instr_supported(instr) {
            return false;
        }
    }

    true
}
