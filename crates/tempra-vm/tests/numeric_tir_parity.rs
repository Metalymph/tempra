//! TIR-only numeric parity: the same verified TIR module is executed by the
//! TIR interpreter and by the TIR -> bytecode -> VM pipeline.
//!
//! `UInt64` values are not source-reachable through the current type checker,
//! so these modules are built directly.

use tempra_bytecode_lower::lower_tir_module;
use tempra_tir::builder::{FunctionBuilder, ModuleBuilder};
use tempra_tir::ir::{TirModule, TirType};
use tempra_tir_interp::interp::interpret_function;
use tempra_vm::{run_function, VmValue};

fn build_module(ret_ty: TirType, build_fn: impl FnOnce(&mut FunctionBuilder)) -> TirModule {
    let mut builder = ModuleBuilder::new();
    let mut function = builder.new_function("test", ret_ty);
    let entry = function.new_block();
    function.set_current_block(entry);
    build_fn(&mut function);
    builder.add_function(function.build());
    builder.module
}

#[derive(Debug, PartialEq)]
enum Normalized {
    Bool(bool),
    UInt64(u64),
    FloatBits(u64),
}

fn canonical_float_bits(value: f64) -> u64 {
    if value.is_nan() {
        f64::NAN.to_bits()
    } else {
        value.to_bits()
    }
}

fn normalize_tir(value: tempra_tir_interp::value::TirRuntimeValue) -> Normalized {
    use tempra_tir_interp::value::TirRuntimeValue;
    match value {
        TirRuntimeValue::Bool(value) => Normalized::Bool(value),
        TirRuntimeValue::UInt64(value) => Normalized::UInt64(value),
        TirRuntimeValue::Float64(value) => Normalized::FloatBits(canonical_float_bits(value)),
        other => panic!("unexpected TIR result: {other:?}"),
    }
}

fn normalize_vm(value: VmValue) -> Normalized {
    match value {
        VmValue::Bool(value) => Normalized::Bool(value),
        VmValue::UInt64(value) => Normalized::UInt64(value),
        VmValue::Float64(value) => Normalized::FloatBits(canonical_float_bits(value)),
        other => panic!("unexpected VM result: {other:?}"),
    }
}

fn run_both(module: &TirModule) -> (Normalized, Normalized) {
    let tir = interpret_function(module, "test", vec![]).expect("TIR interpretation failed");
    let bytecode = lower_tir_module(module).expect("bytecode lowering failed");
    let vm = run_function(&bytecode, "test", vec![]).expect("VM execution failed");
    (normalize_tir(tir), normalize_vm(vm))
}

fn build_uint64_binary(operation: &str, left: u64, right: u64) -> (TirModule, TirType) {
    let ret_ty = if operation == "lt" || operation == "gt" {
        TirType::Bool
    } else {
        TirType::UInt64
    };
    let operation = operation.to_string();
    let module = build_module(ret_ty.clone(), move |function| {
        let left = function
            .build_const(tempra_tir::ir::TirConst::UInt64(left))
            .unwrap();
        let right = function
            .build_const(tempra_tir::ir::TirConst::UInt64(right))
            .unwrap();
        let result = match operation.as_str() {
            "add" => function.build_add(left, right),
            "sub" => function.build_sub(left, right),
            "mul" => function.build_mul(left, right),
            "div" => function.build_div(left, right),
            "lt" => function.build_lt(left, right),
            "gt" => function.build_gt(left, right),
            other => panic!("unknown UInt64 operation {other}"),
        }
        .unwrap();
        function.build_ret(Some(result)).unwrap();
    });
    (module, ret_ty)
}

#[test]
fn uint64_arithmetic_matches_between_tir_interpreter_and_vm() {
    let cases = [
        ("add", 20u64, 22u64, Normalized::UInt64(42)),
        ("sub", 50, 8, Normalized::UInt64(42)),
        ("mul", 21, 2, Normalized::UInt64(42)),
        ("div", 84, 2, Normalized::UInt64(42)),
        ("lt", 1, u64::MAX, Normalized::Bool(true)),
        ("gt", u64::MAX, 1, Normalized::Bool(true)),
    ];

    for (operation, left, right, expected) in cases {
        let (module, _) = build_uint64_binary(operation, left, right);
        let (tir, vm) = run_both(&module);
        assert_eq!(tir, expected, "TIR result for UInt64 {operation}");
        assert_eq!(vm, expected, "VM result for UInt64 {operation}");
    }
}

#[test]
fn uint64_checked_failures_match_between_tir_interpreter_and_vm() {
    let cases = [
        ("add", u64::MAX, 1, "overflow"),
        ("mul", u64::MAX, 2, "overflow"),
        ("sub", 0, 1, "underflow"),
        ("div", 1, 0, "division by zero"),
    ];

    for (operation, left, right, expected_message) in cases {
        let (module, _) = build_uint64_binary(operation, left, right);

        let tir_error =
            interpret_function(&module, "test", vec![]).expect_err("TIR unexpectedly succeeded");
        assert!(
            tir_error.to_string().contains(expected_message),
            "TIR error {tir_error} should contain {expected_message:?}"
        );

        let bytecode = lower_tir_module(&module).expect("bytecode lowering failed");
        let vm_error =
            run_function(&bytecode, "test", vec![]).expect_err("VM unexpectedly succeeded");
        assert!(
            vm_error.to_string().contains(expected_message),
            "VM error {vm_error} should contain {expected_message:?}"
        );
    }
}

#[test]
fn float64_ieee_semantics_match_between_tir_interpreter_and_vm() {
    let cases = [
        (1.0f64, 0.0f64, canonical_float_bits(f64::INFINITY)),
        (-1.0, 0.0, canonical_float_bits(f64::NEG_INFINITY)),
        (1.0, -0.0, canonical_float_bits(f64::NEG_INFINITY)),
        (0.0, 0.0, canonical_float_bits(f64::NAN)),
        (7.0, 2.0, canonical_float_bits(3.5)),
    ];

    for (left, right, expected_bits) in cases {
        let module = build_module(TirType::Float64, move |function| {
            let left = function
                .build_const(tempra_tir::ir::TirConst::Float64(left))
                .unwrap();
            let right = function
                .build_const(tempra_tir::ir::TirConst::Float64(right))
                .unwrap();
            let result = function.build_div(left, right).unwrap();
            function.build_ret(Some(result)).unwrap();
        });

        let (tir, vm) = run_both(&module);
        assert_eq!(tir, Normalized::FloatBits(expected_bits), "TIR Float64 div");
        assert_eq!(vm, Normalized::FloatBits(expected_bits), "VM Float64 div");
    }
}

#[test]
fn float64_nan_ordering_matches_between_tir_interpreter_and_vm() {
    let module = build_module(TirType::Bool, |function| {
        let zero = function
            .build_const(tempra_tir::ir::TirConst::Float64(0.0))
            .unwrap();
        let nan = function.build_div(zero, zero).unwrap();
        let lt = function.build_lt(nan, zero).unwrap();
        let gt = function.build_gt(nan, zero).unwrap();
        let ordered = function.build_or(lt, gt).unwrap();
        function.build_ret(Some(ordered)).unwrap();
    });

    let (tir, vm) = run_both(&module);
    assert_eq!(tir, Normalized::Bool(false), "TIR NaN ordering");
    assert_eq!(vm, Normalized::Bool(false), "VM NaN ordering");
}
