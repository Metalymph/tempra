use crate::error::TirInterpError;
use crate::interp::interpret_function;
use crate::value::TirRuntimeValue;
use tempra_tir::builder::*;
use tempra_tir::ir::*;

fn build_simple_module(
    name: &str,
    ret_ty: TirType,
    build_fn: impl FnOnce(&mut FunctionBuilder),
) -> TirModule {
    let mut mb = ModuleBuilder::new();
    let mut fb = mb.new_function(name, ret_ty);
    let entry = fb.new_block();
    fb.set_current_block(entry);
    build_fn(&mut fb);
    mb.add_function(fb.build());
    mb.module
}

#[test]
fn interpret_const_return_i64() {
    let module = build_simple_module("test", TirType::Int64, |fb| {
        let c = fb.build_const(TirConst::Int64(42)).unwrap();
        fb.build_ret(Some(c)).unwrap();
    });
    let result = interpret_function(&module, "test", vec![]).unwrap();
    assert_eq!(result, TirRuntimeValue::Int64(42));
}

#[test]
fn interpret_add_i64() {
    let module = build_simple_module("test", TirType::Int64, |fb| {
        let a = fb.build_const(TirConst::Int64(10)).unwrap();
        let b = fb.build_const(TirConst::Int64(32)).unwrap();
        let r = fb.build_add(a, b).unwrap();
        fb.build_ret(Some(r)).unwrap();
    });
    let result = interpret_function(&module, "test", vec![]).unwrap();
    assert_eq!(result, TirRuntimeValue::Int64(42));
}

#[test]
fn interpret_bool_and() {
    let module = build_simple_module("test", TirType::Bool, |fb| {
        let a = fb.build_const(TirConst::Bool(true)).unwrap();
        let b = fb.build_const(TirConst::Bool(false)).unwrap();
        let r = fb.build_and(a, b).unwrap();
        fb.build_ret(Some(r)).unwrap();
    });
    let result = interpret_function(&module, "test", vec![]).unwrap();
    assert_eq!(result, TirRuntimeValue::Bool(false));
}

#[test]
fn interpret_if_condbr() {
    let mut mb = ModuleBuilder::new();
    let mut fb = mb.new_function("test", TirType::Int64);
    let entry = fb.new_block();
    let then_b = fb.new_block();
    let else_b = fb.new_block();

    fb.set_current_block(entry);
    let cond = fb.build_const(TirConst::Bool(true)).unwrap();
    fb.build_condbr(cond, then_b, else_b).unwrap();

    fb.set_current_block(then_b);
    let v1 = fb.build_const(TirConst::Int64(100)).unwrap();
    fb.build_ret(Some(v1)).unwrap();

    fb.set_current_block(else_b);
    let v2 = fb.build_const(TirConst::Int64(200)).unwrap();
    fb.build_ret(Some(v2)).unwrap();

    mb.add_function(fb.build());
    let module = mb.module;

    let result = interpret_function(&module, "test", vec![]).unwrap();
    assert_eq!(result, TirRuntimeValue::Int64(100));
}

#[test]
fn interpret_slot_load_store() {
    let module = build_simple_module("test", TirType::Int64, |fb| {
        let slot = fb.build_slot(TirType::Int64, Some("x")).unwrap();
        let v1 = fb.build_const(TirConst::Int64(10)).unwrap();
        fb.build_store(slot, v1).unwrap();
        let v2 = fb.build_const(TirConst::Int64(20)).unwrap();
        fb.build_store(slot, v2).unwrap();
        let loaded = fb.build_load(slot).unwrap();
        fb.build_ret(Some(loaded)).unwrap();
    });
    let result = interpret_function(&module, "test", vec![]).unwrap();
    assert_eq!(result, TirRuntimeValue::Int64(20));
}

#[test]
fn interpret_param_identity() {
    let mut mb = ModuleBuilder::new();
    let mut fb = mb.new_function("id", TirType::Int64);
    let param = fb.add_param(TirType::Int64);
    let entry = fb.new_block();
    fb.set_current_block(entry);
    fb.build_ret(Some(param)).unwrap();
    mb.add_function(fb.build());
    let module = mb.module;

    let result = interpret_function(&module, "id", vec![TirRuntimeValue::Int64(99)]).unwrap();
    assert_eq!(result, TirRuntimeValue::Int64(99));
}

#[test]
fn interpret_call_simple_add() {
    let mut mb = ModuleBuilder::new();

    // Build `add(a, b) -> a + b`
    let mut fb_add = mb.new_function("add", TirType::Int64);
    let pa = fb_add.add_param(TirType::Int64);
    let pb = fb_add.add_param(TirType::Int64);
    let entry_add = fb_add.new_block();
    fb_add.set_current_block(entry_add);
    let sum = fb_add.build_add(pa, pb).unwrap();
    fb_add.build_ret(Some(sum)).unwrap();
    mb.add_function(fb_add.build());

    // Build `main() -> call add(10, 20)`
    let add_id = mb.declare_function("add");
    let mut fb_main = mb.new_function("main", TirType::Int64);
    let entry_main = fb_main.new_block();
    fb_main.set_current_block(entry_main);
    let c1 = fb_main.build_const(TirConst::Int64(10)).unwrap();
    let c2 = fb_main.build_const(TirConst::Int64(20)).unwrap();
    let call_result = fb_main.build_call(add_id, vec![c1, c2]).unwrap();
    fb_main.build_ret(Some(call_result)).unwrap();
    mb.add_function(fb_main.build());

    let module = mb.module;

    let result = interpret_function(&module, "main", vec![]).unwrap();
    assert_eq!(result, TirRuntimeValue::Int64(30));
}

#[test]
fn interpret_division_by_zero_error() {
    let module = build_simple_module("test", TirType::Int64, |fb| {
        let a = fb.build_const(TirConst::Int64(10)).unwrap();
        let b = fb.build_const(TirConst::Int64(0)).unwrap();
        let r = fb.build_div(a, b).unwrap();
        fb.build_ret(Some(r)).unwrap();
    });
    let result = interpret_function(&module, "test", vec![]);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        TirInterpError::Int64Arithmetic(tempra_runtime::Int64ArithmeticError::DivisionByZero)
    ));
}

#[test]
fn interpret_int64_arithmetic_failures_are_checked() {
    for (operation, left, right, expected) in [
        (
            "add",
            i64::MAX,
            1,
            tempra_runtime::Int64ArithmeticError::Overflow,
        ),
        (
            "sub",
            i64::MIN,
            1,
            tempra_runtime::Int64ArithmeticError::Overflow,
        ),
        (
            "mul",
            i64::MAX,
            2,
            tempra_runtime::Int64ArithmeticError::Overflow,
        ),
        (
            "div",
            i64::MIN,
            -1,
            tempra_runtime::Int64ArithmeticError::Overflow,
        ),
    ] {
        let module = build_simple_module("test", TirType::Int64, |fb| {
            let left = fb.build_const(TirConst::Int64(left)).unwrap();
            let right = fb.build_const(TirConst::Int64(right)).unwrap();
            let result = match operation {
                "add" => fb.build_add(left, right),
                "sub" => fb.build_sub(left, right),
                "mul" => fb.build_mul(left, right),
                "div" => fb.build_div(left, right),
                _ => unreachable!(),
            }
            .unwrap();
            fb.build_ret(Some(result)).unwrap();
        });
        assert!(matches!(
            interpret_function(&module, "test", vec![]),
            Err(TirInterpError::Int64Arithmetic(error)) if error == expected
        ));
    }
}

#[test]
fn interpret_unsupported_make_error() {
    let module = build_simple_module("test", TirType::Unit, |fb| {
        let _ = fb
            .build_make(TirType::Struct("Foo".into()), None, vec![])
            .unwrap();
        fb.build_ret(None).unwrap();
    });
    let result = interpret_function(&module, "test", vec![]);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        TirInterpError::UnsupportedInstr(_)
    ));
}

#[test]
fn interpret_invalid_tir_is_rejected_by_verifier_before_execution() {
    let module = build_simple_module("test", TirType::Unit, |fb| {
        // If execution happened first, this unsupported Make would report
        // UnsupportedInstr. The missing terminator makes the module invalid, so
        // verification must reject it first.
        let _ = fb
            .build_make(TirType::Struct("Foo".into()), None, vec![])
            .unwrap();
    });

    match interpret_function(&module, "test", vec![]) {
        Err(TirInterpError::VerifierFailed(errors)) => {
            assert!(
                errors.contains("MissingReturn"),
                "expected a missing-return verifier error, got: {errors}"
            );
        }
        other => panic!("expected VerifierFailed before execution, got {other:?}"),
    }
}

#[test]
fn interpret_unreachable_error() {
    let module = build_simple_module("test", TirType::Unit, |fb| {
        fb.build_unreachable().unwrap();
    });
    let result = interpret_function(&module, "test", vec![]);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        TirInterpError::UnreachableExecuted
    ));
}

#[test]
fn interpret_uint64_arithmetic_is_checked() {
    for (operation, left, right, expected) in [
        ("add", 20u64, 22u64, 42u64),
        ("sub", 50, 8, 42),
        ("mul", 21, 2, 42),
        ("div", 84, 2, 42),
    ] {
        let module = build_simple_module("test", TirType::UInt64, |fb| {
            let left = fb.build_const(TirConst::UInt64(left)).unwrap();
            let right = fb.build_const(TirConst::UInt64(right)).unwrap();
            let result = match operation {
                "add" => fb.build_add(left, right),
                "sub" => fb.build_sub(left, right),
                "mul" => fb.build_mul(left, right),
                "div" => fb.build_div(left, right),
                _ => unreachable!(),
            }
            .unwrap();
            fb.build_ret(Some(result)).unwrap();
        });
        assert_eq!(
            interpret_function(&module, "test", vec![]).unwrap(),
            TirRuntimeValue::UInt64(expected)
        );
    }

    for (operation, left, right, expected) in [
        (
            "add",
            u64::MAX,
            1,
            tempra_runtime::UInt64ArithmeticError::Overflow,
        ),
        (
            "mul",
            u64::MAX,
            2,
            tempra_runtime::UInt64ArithmeticError::Overflow,
        ),
        (
            "sub",
            0,
            1,
            tempra_runtime::UInt64ArithmeticError::Underflow,
        ),
        (
            "div",
            1,
            0,
            tempra_runtime::UInt64ArithmeticError::DivisionByZero,
        ),
    ] {
        let module = build_simple_module("test", TirType::UInt64, |fb| {
            let left = fb.build_const(TirConst::UInt64(left)).unwrap();
            let right = fb.build_const(TirConst::UInt64(right)).unwrap();
            let result = match operation {
                "add" => fb.build_add(left, right),
                "sub" => fb.build_sub(left, right),
                "mul" => fb.build_mul(left, right),
                "div" => fb.build_div(left, right),
                _ => unreachable!(),
            }
            .unwrap();
            fb.build_ret(Some(result)).unwrap();
        });
        assert!(matches!(
            interpret_function(&module, "test", vec![]),
            Err(TirInterpError::UInt64Arithmetic(error)) if error == expected
        ));
    }
}

#[test]
fn interpret_uint64_comparisons_use_unsigned_ordering() {
    let module = build_simple_module("test", TirType::Bool, |fb| {
        let large = fb.build_const(TirConst::UInt64(u64::MAX)).unwrap();
        let one = fb.build_const(TirConst::UInt64(1)).unwrap();
        let gt = fb.build_gt(large, one).unwrap();
        let lt = fb.build_lt(one, large).unwrap();
        let ordered = fb.build_and(gt, lt).unwrap();
        fb.build_ret(Some(ordered)).unwrap();
    });
    assert_eq!(
        interpret_function(&module, "test", vec![]).unwrap(),
        TirRuntimeValue::Bool(true)
    );
}

#[test]
fn interpret_float64_preserves_ieee754_semantics() {
    // +0 and -0 divide differently.
    let positive = build_simple_module("positive", TirType::Float64, |fb| {
        let one = fb.build_const(TirConst::Float64(1.0)).unwrap();
        let zero = fb.build_const(TirConst::Float64(0.0)).unwrap();
        let result = fb.build_div(one, zero).unwrap();
        fb.build_ret(Some(result)).unwrap();
    });
    assert_eq!(
        interpret_function(&positive, "positive", vec![]).unwrap(),
        TirRuntimeValue::Float64(f64::INFINITY)
    );

    let negative = build_simple_module("negative", TirType::Float64, |fb| {
        let one = fb.build_const(TirConst::Float64(1.0)).unwrap();
        let zero = fb.build_const(TirConst::Float64(-0.0)).unwrap();
        let result = fb.build_div(one, zero).unwrap();
        fb.build_ret(Some(result)).unwrap();
    });
    assert_eq!(
        interpret_function(&negative, "negative", vec![]).unwrap(),
        TirRuntimeValue::Float64(f64::NEG_INFINITY)
    );

    // NaN from 0/0 is not equal to itself and neither ordering comparison holds.
    let nan = build_simple_module("nan", TirType::Bool, |fb| {
        let zero = fb.build_const(TirConst::Float64(0.0)).unwrap();
        let nan = fb.build_div(zero, zero).unwrap();
        let eq = fb.build_eq(nan, nan).unwrap();
        let lt = fb.build_lt(nan, zero).unwrap();
        let gt = fb.build_gt(nan, zero).unwrap();
        let neither = fb.build_not(eq).unwrap();
        let lt_false = fb.build_not(lt).unwrap();
        let both = fb.build_and(neither, lt_false).unwrap();
        let gt_false = fb.build_not(gt).unwrap();
        let all = fb.build_and(both, gt_false).unwrap();
        fb.build_ret(Some(all)).unwrap();
    });
    assert_eq!(
        interpret_function(&nan, "nan", vec![]).unwrap(),
        TirRuntimeValue::Bool(true)
    );

    // Equality distinguishes NaN but not signed zero.
    let equality = build_simple_module("equality", TirType::Bool, |fb| {
        let zero = fb.build_const(TirConst::Float64(0.0)).unwrap();
        let negative_zero = fb.build_const(TirConst::Float64(-0.0)).unwrap();
        let zero_eq = fb.build_eq(zero, negative_zero).unwrap();
        let nan = fb.build_div(zero, zero).unwrap();
        let nan_eq = fb.build_eq(nan, nan).unwrap();
        let neq = fb.build_not(nan_eq).unwrap();
        let result = fb.build_and(zero_eq, neq).unwrap();
        fb.build_ret(Some(result)).unwrap();
    });
    assert_eq!(
        interpret_function(&equality, "equality", vec![]).unwrap(),
        TirRuntimeValue::Bool(true)
    );
}

#[test]
fn interpret_float64_negation_by_minus_one_preserves_signed_zero_and_infinity() {
    let module = build_simple_module("negate", TirType::Bool, |fb| {
        let minus_one = fb.build_const(TirConst::Float64(-1.0)).unwrap();
        let zero = fb.build_const(TirConst::Float64(0.0)).unwrap();
        let negative_zero = fb.build_mul(minus_one, zero).unwrap();
        let infinity = fb.build_const(TirConst::Float64(f64::INFINITY)).unwrap();
        let negative_infinity = fb.build_mul(minus_one, infinity).unwrap();
        let one = fb.build_const(TirConst::Float64(1.0)).unwrap();
        let signed_zero_division = fb.build_div(one, negative_zero).unwrap();
        let neg_inf_is_neg_inf = fb
            .build_eq(negative_infinity, signed_zero_division)
            .unwrap();
        let positive_zero = fb.build_const(TirConst::Float64(0.0)).unwrap();
        let zero_is_negative = fb.build_lt(negative_zero, positive_zero).unwrap();
        let zero_not_negative = fb.build_not(zero_is_negative).unwrap();
        let result = fb.build_and(neg_inf_is_neg_inf, zero_not_negative).unwrap();
        fb.build_ret(Some(result)).unwrap();
    });
    assert_eq!(
        interpret_function(&module, "negate", vec![]).unwrap(),
        TirRuntimeValue::Bool(true)
    );
}
