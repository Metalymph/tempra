#![cfg(feature = "jit-cranelift")]

use std::process::Command;
use tempra_bytecode::bytecode::BytecodeModule;
use tempra_bytecode::function::{BytecodeFunction, BytecodeType, FunctionId, LocalId};
use tempra_bytecode::opcode::BytecodeInstr;
use tempra_vm::profile::VmOptions;
use tempra_vm::value::VmValue;
use tempra_vm::Interpreter;

fn build_pair_len(module: &mut BytecodeModule) -> FunctionId {
    let mut func = BytecodeFunction::new(
        "pair_len".into(),
        vec![BytecodeType::String, BytecodeType::String],
        BytecodeType::Int64,
    );
    func.locals = vec![BytecodeType::String, BytecodeType::String];
    func.code.extend([
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::Len,
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::Len,
        BytecodeInstr::AddI64,
        BytecodeInstr::Return,
    ]);
    func.max_stack = 2;
    module.add_function(func)
}

fn run_jit_probe() {
    let mut module = BytecodeModule::new();
    let func_id = build_pair_len(&mut module);

    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.profiling = true;
    options.hot_call_threshold = 0;
    options.jit_min_instruction_count = 0;

    let mut interpreter = Interpreter::with_options(&module, options);
    for _ in 0..2 {
        let result = interpreter
            .run_function_id(
                func_id,
                vec![
                    VmValue::String("tempra".into()),
                    VmValue::String("tempra".into()),
                ],
            )
            .unwrap();
        assert_eq!(result, VmValue::Int64(12));
    }

    // The warm call marshals duplicate strings through the helper cache,
    // producing both a miss and a hit.
    let profile = interpreter.build_profile();
    let stats = &profile.function_stats[func_id.0 as usize];
    assert!(stats.jit_heap_runtime_calls > 0);
    assert_eq!(stats.jit_heap_handle_cache_misses, 1);
    assert_eq!(stats.jit_heap_handle_cache_hits, 1);

    println!("TEMPRA_JIT_OUTPUT_SENTINEL");
}

#[test]
fn jit_helper_cache_does_not_pollute_stdout() {
    if std::env::var_os("TEMPRA_JIT_OUTPUT_CHILD").is_some() {
        run_jit_probe();
        return;
    }

    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "jit_helper_cache_does_not_pollute_stdout",
            "--nocapture",
        ])
        .env("TEMPRA_JIT_OUTPUT_CHILD", "1")
        .output()
        .expect("failed to spawn JIT output child");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "JIT output child failed: stdout={stdout}\nstderr={stderr}"
    );
    assert!(
        stdout.contains("TEMPRA_JIT_OUTPUT_SENTINEL"),
        "child did not execute the JIT probe:\n{stdout}"
    );
    for marker in ["alloc_or_get_string", "HIT!", "MISS!"] {
        assert!(
            !stdout.contains(marker),
            "JIT helper wrote internal diagnostic {marker:?} to stdout:\n{stdout}"
        );
    }
}
