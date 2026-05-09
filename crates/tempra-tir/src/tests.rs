use super::ir::*;
use super::builder::*;
use super::verifier::*;
use super::pretty::*;

#[test]
fn test_const_ok() {
    let mut builder = ModuleBuilder::new();
    let mut func = builder.new_function("main", TirType::Unit);
    let b1 = func.new_block();
    func.set_current_block(b1);
    
    let _c = func.build_const(TirConst::Int64(42)).unwrap();
    func.build_ret(None).unwrap();
    
    builder.add_function(func.build());
    let module = builder.module;
    assert!(verify_module(&module).is_ok());
}

#[test]
fn test_add_i64_ok() {
    let mut builder = ModuleBuilder::new();
    let mut func = builder.new_function("main", TirType::Int64);
    let b1 = func.new_block();
    func.set_current_block(b1);
    
    let a = func.build_const(TirConst::Int64(10)).unwrap();
    let b = func.build_const(TirConst::Int64(20)).unwrap();
    let res = func.build_add(a, b).unwrap();
    func.build_ret(Some(res)).unwrap();
    
    builder.add_function(func.build());
    let module = builder.module;
    assert!(verify_module(&module).is_ok());
}

#[test]
fn test_add_type_mismatch() {
    let mut builder = ModuleBuilder::new();
    let mut func = builder.new_function("main", TirType::Unit);
    let b1 = func.new_block();
    func.set_current_block(b1);
    
    let a = func.build_const(TirConst::Int64(10)).unwrap();
    let b = func.build_const(TirConst::Bool(true)).unwrap();
    let _res = func.build_add(a, b).unwrap();
    func.build_ret(None).unwrap();
    
    builder.add_function(func.build());
    let module = builder.module;
    let errs = verify_module(&module).unwrap_err();
    assert!(errs.contains(&VerifierError::TypeMismatch {
        expected: TirType::Int64,
        actual: TirType::Bool,
    }));
}

#[test]
fn test_slot_load_store_ok() {
    let mut builder = ModuleBuilder::new();
    let mut func = builder.new_function("main", TirType::Int64);
    let b1 = func.new_block();
    func.set_current_block(b1);
    
    let slot = func.build_slot(TirType::Int64, Some("x")).unwrap();
    let a = func.build_const(TirConst::Int64(100)).unwrap();
    func.build_store(slot, a).unwrap();
    let loaded = func.build_load(slot).unwrap();
    func.build_ret(Some(loaded)).unwrap();
    
    builder.add_function(func.build());
    assert!(verify_module(&builder.module).is_ok());
}

#[test]
fn test_store_mismatch() {
    let mut builder = ModuleBuilder::new();
    let mut func = builder.new_function("main", TirType::Unit);
    let b1 = func.new_block();
    func.set_current_block(b1);
    
    let slot = func.build_slot(TirType::Int64, Some("x")).unwrap();
    let a = func.build_const(TirConst::Bool(true)).unwrap();
    func.build_store(slot, a).unwrap();
    func.build_ret(None).unwrap();
    
    builder.add_function(func.build());
    let errs = verify_module(&builder.module).unwrap_err();
    assert!(errs.contains(&VerifierError::TypeMismatch {
        expected: TirType::Int64,
        actual: TirType::Bool,
    }));
}

#[test]
fn test_return_type_ok() {
    let mut builder = ModuleBuilder::new();
    let mut func = builder.new_function("main", TirType::Bool);
    let b1 = func.new_block();
    func.set_current_block(b1);
    
    let c = func.build_const(TirConst::Bool(false)).unwrap();
    func.build_ret(Some(c)).unwrap();
    
    builder.add_function(func.build());
    assert!(verify_module(&builder.module).is_ok());
}

#[test]
fn test_return_type_error() {
    let mut builder = ModuleBuilder::new();
    let mut func = builder.new_function("main", TirType::Bool);
    let b1 = func.new_block();
    func.set_current_block(b1);
    
    let c = func.build_const(TirConst::Int64(0)).unwrap();
    func.build_ret(Some(c)).unwrap();
    
    builder.add_function(func.build());
    let errs = verify_module(&builder.module).unwrap_err();
    assert!(errs.contains(&VerifierError::TypeMismatch {
        expected: TirType::Bool,
        actual: TirType::Int64,
    }));
}

#[test]
fn test_condbr_bool_ok() {
    let mut builder = ModuleBuilder::new();
    let mut func = builder.new_function("main", TirType::Unit);
    let b1 = func.new_block();
    let b2 = func.new_block();
    let b3 = func.new_block();
    
    func.set_current_block(b1);
    let cond = func.build_const(TirConst::Bool(true)).unwrap();
    func.build_condbr(cond, b2, b3).unwrap();
    
    func.set_current_block(b2);
    func.build_ret(None).unwrap();
    
    func.set_current_block(b3);
    func.build_ret(None).unwrap();
    
    builder.add_function(func.build());
    assert!(verify_module(&builder.module).is_ok());
}

#[test]
fn test_condbr_non_bool_error() {
    let mut builder = ModuleBuilder::new();
    let mut func = builder.new_function("main", TirType::Unit);
    let b1 = func.new_block();
    let b2 = func.new_block();
    
    func.set_current_block(b1);
    let cond = func.build_const(TirConst::Int64(1)).unwrap();
    func.build_condbr(cond, b2, b2).unwrap();
    
    func.set_current_block(b2);
    func.build_ret(None).unwrap();
    
    builder.add_function(func.build());
    let errs = verify_module(&builder.module).unwrap_err();
    assert!(errs.contains(&VerifierError::TypeMismatch {
        expected: TirType::Bool,
        actual: TirType::Int64,
    }));
}

#[test]
fn test_branch_missing_block_error() {
    let mut builder = ModuleBuilder::new();
    let mut func = builder.new_function("main", TirType::Unit);
    let b1 = func.new_block();
    func.set_current_block(b1);
    func.build_br(TirBlockId(999)).unwrap();
    
    builder.add_function(func.build());
    let errs = verify_module(&builder.module).unwrap_err();
    assert!(errs.contains(&VerifierError::InvalidBranchTarget(TirBlockId(999))));
}

#[test]
fn test_make_get_set_smoke() {
    let mut builder = ModuleBuilder::new();
    let mut func = builder.new_function("main", TirType::Unit);
    let b1 = func.new_block();
    func.set_current_block(b1);
    
    let t = TirType::Struct("MyStruct".to_string());
    let val = func.build_const(TirConst::Int64(42)).unwrap();
    let obj = func.build_make(t, vec![val]).unwrap();
    let _extracted = func.build_get(obj, TirKey::Field("field".to_string())).unwrap();
    func.build_set(obj, TirKey::Field("field".to_string()), val).unwrap();
    
    func.build_ret(None).unwrap();
    builder.add_function(func.build());
    assert!(verify_module(&builder.module).is_ok());
}

#[test]
fn test_tag_unwrap_smoke() {
    let mut builder = ModuleBuilder::new();
    let mut func = builder.new_function("main", TirType::Unit);
    let b1 = func.new_block();
    func.set_current_block(b1);
    
    let t = TirType::Enum("MyEnum".to_string());
    let obj = func.build_make(t, vec![]).unwrap();
    let _tag = func.build_tag(obj).unwrap();
    let _unwrapped = func.build_unwrap(obj).unwrap();
    
    func.build_ret(None).unwrap();
    builder.add_function(func.build());
    assert!(verify_module(&builder.module).is_ok());
}

#[test]
fn test_pretty_printer() {
    let mut builder = ModuleBuilder::new();
    let mut func = builder.new_function("example", TirType::Int64);
    func.add_param(TirType::Int64);
    
    let b1 = func.new_block();
    func.set_current_block(b1);
    let s = func.build_slot(TirType::Int64, Some("acc")).unwrap();
    let a = func.build_const(TirConst::Int64(10)).unwrap();
    func.build_store(s, a).unwrap();
    let l = func.build_load(s).unwrap();
    func.build_ret(Some(l)).unwrap();
    
    builder.add_function(func.build());
    
    let out = format_tir_module(&builder.module);
    assert!(out.contains("fn example (%0: i64) -> i64 {"));
    assert!(out.contains("// slot $0 = acc"));
    assert!(out.contains("%1 = const 10i64"));
    assert!(out.contains("store $0, %1"));
}
