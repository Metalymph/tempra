use tempra_tir::builder::*;
use tempra_tir::ir::*;
use crate::interp::interpret_function;
use crate::value::TirRuntimeValue;
use crate::error::TirInterpError;

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
    assert!(matches!(result.unwrap_err(), TirInterpError::DivisionByZero));
}

#[test]
fn interpret_unsupported_make_error() {
    let module = build_simple_module("test", TirType::Unit, |fb| {
        let _ = fb.build_make(TirType::Struct("Foo".into()), None, vec![]).unwrap();
        fb.build_ret(None).unwrap();
    });
    let result = interpret_function(&module, "test", vec![]);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), TirInterpError::UnsupportedInstr(_)));
}

#[test]
fn interpret_unreachable_error() {
    let module = build_simple_module("test", TirType::Unit, |fb| {
        fb.build_unreachable().unwrap();
    });
    let result = interpret_function(&module, "test", vec![]);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), TirInterpError::UnreachableExecuted));
}
