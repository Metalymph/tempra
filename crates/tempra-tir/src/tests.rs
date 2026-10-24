use super::builder::*;
use super::ir::*;
use super::pretty::*;
use super::verifier::*;

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
    let obj = func.build_make(t, None, vec![val]).unwrap();
    let _extracted = func
        .build_get(obj, TirKey::Field("field".to_string()))
        .unwrap();
    func.build_set(obj, TirKey::Field("field".to_string()), val)
        .unwrap();

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
    let obj = func.build_make(t, None, vec![]).unwrap();
    let _tag = func.build_tag(obj).unwrap();
    let _unwrapped = func.build_unwrap(obj, "MyVariant".to_string(), 0).unwrap();

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

#[test]
fn tag_is_a_string_but_comparisons_remain_same_type() {
    for compare_to_string in [true, false] {
        let mut module = ModuleBuilder::new();
        let mut function = module.new_function("tag_compare", TirType::Bool);
        let entry = function.new_block();
        function.set_current_block(entry);
        let value = function
            .build_make(
                TirType::Option(Box::new(TirType::Int64)),
                Some("None".into()),
                vec![],
            )
            .unwrap();
        let tag = function.build_tag(value).unwrap();
        let literal = if compare_to_string {
            TirConst::String("None".into())
        } else {
            TirConst::Int64(0)
        };
        let wanted = function.build_const(literal).unwrap();
        let condition = function.build_eq(tag, wanted).unwrap();
        function.build_ret(Some(condition)).unwrap();
        module.add_function(function.build());
        if compare_to_string {
            assert!(verify_module(&module.module).is_ok());
        } else {
            assert!(verify_module(&module.module).unwrap_err().contains(
                &VerifierError::TypeMismatch {
                    expected: TirType::String,
                    actual: TirType::Int64,
                },
            ));
        }
    }
}

#[test]
fn mismatched_result_store_and_return_are_still_rejected() {
    let expected = TirType::Result(Box::new(TirType::Int64), Box::new(TirType::String));
    let wrong = TirType::Result(Box::new(TirType::Int64), Box::new(TirType::Unit));
    let mut module = ModuleBuilder::new();
    let mut function = module.new_function("wrong_result", expected.clone());
    let entry = function.new_block();
    function.set_current_block(entry);
    let payload = function.build_const(TirConst::Int64(1)).unwrap();
    let value = function
        .build_make(wrong.clone(), Some("Ok".into()), vec![payload])
        .unwrap();
    let slot = function.build_slot(expected.clone(), None).unwrap();
    function.build_store(slot, value).unwrap();
    function.build_ret(Some(value)).unwrap();
    module.add_function(function.build());
    let mismatch = VerifierError::TypeMismatch {
        expected,
        actual: wrong,
    };
    let errors = verify_module(&module.module).unwrap_err();
    assert_eq!(errors.iter().filter(|error| **error == mismatch).count(), 2);
}

#[test]
fn typed_verification_resolves_uint64_and_float64_arithmetic() {
    let mut module = ModuleBuilder::new();
    let mut function = module.new_function("typed_arithmetic", TirType::Unit);
    let entry = function.new_block();
    function.set_current_block(entry);

    let u64_left = function.build_const(TirConst::UInt64(1)).unwrap();
    let u64_right = function.build_const(TirConst::UInt64(2)).unwrap();
    let u64_sum = function.build_add(u64_left, u64_right).unwrap();

    let f64_left = function.build_const(TirConst::Float64(1.5)).unwrap();
    let f64_right = function.build_const(TirConst::Float64(2.5)).unwrap();
    let f64_sum = function.build_add(f64_left, f64_right).unwrap();
    let ordered = function.build_lt(f64_left, f64_right).unwrap();
    let equal = function.build_eq(f64_left, f64_right).unwrap();

    function.build_ret(None).unwrap();
    let built = function.build();
    let function_id = built.id;
    module.add_function(built);

    let verified = match verify_module_with_types(&module.module) {
        Ok(verified) => verified,
        Err(errors) => panic!("typed verification failed: {errors:?}"),
    };
    let types = verified.functions.get(&function_id).unwrap();
    assert_eq!(types.get(&u64_sum), Some(&TirType::UInt64));
    assert_eq!(types.get(&f64_sum), Some(&TirType::Float64));
    assert_eq!(types.get(&ordered), Some(&TirType::Bool));
    assert_eq!(types.get(&equal), Some(&TirType::Bool));
}

#[test]
fn non_numeric_and_unresolved_arithmetic_types_are_rejected() {
    let mut module = ModuleBuilder::new();
    let mut function = module.new_function("bad_arithmetic", TirType::Unit);
    let entry = function.new_block();
    function.set_current_block(entry);

    let text = function.build_const(TirConst::String("x".into())).unwrap();
    let number = function.build_const(TirConst::Int64(1)).unwrap();
    let _invalid = function.build_add(text, text).unwrap();
    let _mismatch = function.build_sub(number, text).unwrap();

    let unknown_member = function.build_make_struct("Empty".into(), vec![]).unwrap();
    let unresolved = function
        .build_get(unknown_member, TirKey::Field("missing".into()))
        .unwrap();
    let _unresolved_add = function.build_add(unresolved, number).unwrap();

    function.build_ret(None).unwrap();
    module.add_function(function.build());

    let errors = verify_module(&module.module).unwrap_err();
    assert!(errors.contains(&VerifierError::InvalidArithmeticType(TirType::String)));
    assert!(errors.contains(&VerifierError::TypeMismatch {
        expected: TirType::Int64,
        actual: TirType::String,
    }));
    assert!(errors.contains(&VerifierError::UnresolvedValueType(unresolved)));
}

#[test]
fn ordering_comparisons_require_numeric_types_but_equality_does_not() {
    let mut module = ModuleBuilder::new();
    let mut function = module.new_function("bad_comparison", TirType::Unit);
    let entry = function.new_block();
    function.set_current_block(entry);

    let left = function.build_const(TirConst::Bool(true)).unwrap();
    let right = function.build_const(TirConst::Bool(false)).unwrap();
    let _ordered = function.build_lt(left, right).unwrap();
    let _equal = function.build_eq(left, right).unwrap();

    function.build_ret(None).unwrap();
    module.add_function(function.build());

    let errors = verify_module(&module.module).unwrap_err();
    assert!(errors.contains(&VerifierError::InvalidComparisonType(TirType::Bool)));
    assert!(!errors.contains(&VerifierError::InvalidArithmeticType(TirType::Bool)));
}

// K3-H1: struct field inference must reject conflicting nominal layouts and
// must retain field discoveries made earlier in the same inference pass.

fn conflict_pairs(errors: &[VerifierError]) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = errors
        .iter()
        .filter_map(|error| match error {
            VerifierError::StructFieldTypeConflict { struct_name, field } => {
                Some((struct_name.clone(), field.clone()))
            }
            _ => None,
        })
        .collect();
    pairs.sort();
    pairs
}

fn expected_conflict(struct_name: &str, field: &str) -> (String, String) {
    (struct_name.to_string(), field.to_string())
}

fn conflicting_counter_in_one_function(float_first: bool) -> TirModule {
    let mut module = ModuleBuilder::new();
    let mut function = module.new_function("main", TirType::Unit);
    let entry = function.new_block();
    function.set_current_block(entry);

    let values = if float_first {
        [TirConst::Float64(1.5), TirConst::Int64(1)]
    } else {
        [TirConst::Int64(1), TirConst::Float64(1.5)]
    };
    for value in values {
        let value = function.build_const(value).unwrap();
        function
            .build_make_struct("Counter".into(), vec![("value".into(), value)])
            .unwrap();
    }

    function.build_ret(None).unwrap();
    module.add_function(function.build());
    module.module
}

fn conflicting_counter_across_functions(float_first: bool) -> TirModule {
    let mut module = ModuleBuilder::new();

    let mut make_int = module.new_function("make_int", TirType::Struct("Counter".into()));
    let entry = make_int.new_block();
    make_int.set_current_block(entry);
    let int_value = make_int.build_const(TirConst::Int64(1)).unwrap();
    let int_struct = make_int
        .build_make_struct("Counter".into(), vec![("value".into(), int_value)])
        .unwrap();
    make_int.build_ret(Some(int_struct)).unwrap();
    let int_function = make_int.build();

    let mut make_float = module.new_function("make_float", TirType::Struct("Counter".into()));
    let entry = make_float.new_block();
    make_float.set_current_block(entry);
    let float_value = make_float.build_const(TirConst::Float64(1.5)).unwrap();
    let float_struct = make_float
        .build_make_struct("Counter".into(), vec![("value".into(), float_value)])
        .unwrap();
    make_float.build_ret(Some(float_struct)).unwrap();
    let float_function = make_float.build();

    if float_first {
        module.add_function(float_function);
        module.add_function(int_function);
    } else {
        module.add_function(int_function);
        module.add_function(float_function);
    }
    module.module
}

#[test]
fn struct_field_conflict_in_one_function_is_rejected() {
    let module = conflicting_counter_in_one_function(false);
    let errors = verify_module(&module).unwrap_err();
    assert_eq!(
        conflict_pairs(&errors),
        vec![expected_conflict("Counter", "value")]
    );
}

#[test]
fn struct_field_conflict_detection_is_order_independent() {
    for float_first in [false, true] {
        let module = conflicting_counter_in_one_function(float_first);
        let errors = verify_module(&module).unwrap_err();
        assert_eq!(
            conflict_pairs(&errors),
            vec![expected_conflict("Counter", "value")],
            "float_first={float_first}"
        );
    }
}

#[test]
fn struct_field_conflict_across_functions_is_rejected() {
    for float_first in [false, true] {
        let module = conflicting_counter_across_functions(float_first);
        let errors = verify_module(&module).unwrap_err();
        assert_eq!(
            conflict_pairs(&errors),
            vec![expected_conflict("Counter", "value")],
            "float_first={float_first}"
        );
    }
}

#[test]
fn valid_repeated_struct_constructions_are_accepted() {
    for reverse in [false, true] {
        let mut module = ModuleBuilder::new();

        let mut make_one = module.new_function("make_one", TirType::Struct("Counter".into()));
        let entry = make_one.new_block();
        make_one.set_current_block(entry);
        let one = make_one.build_const(TirConst::Int64(1)).unwrap();
        let first = make_one
            .build_make_struct("Counter".into(), vec![("value".into(), one)])
            .unwrap();
        make_one.build_ret(Some(first)).unwrap();
        let make_one = make_one.build();

        let mut make_two = module.new_function("make_two", TirType::Struct("Counter".into()));
        let entry = make_two.new_block();
        make_two.set_current_block(entry);
        let two = make_two.build_const(TirConst::Int64(2)).unwrap();
        let second = make_two
            .build_make_struct("Counter".into(), vec![("value".into(), two)])
            .unwrap();
        make_two.build_ret(Some(second)).unwrap();
        let make_two = make_two.build();

        let mut read = module.new_function("read", TirType::Int64);
        let counter = read.add_param(TirType::Struct("Counter".into()));
        let entry = read.new_block();
        read.set_current_block(entry);
        let field = read
            .build_get(counter, TirKey::Field("value".into()))
            .unwrap();
        let one = read.build_const(TirConst::Int64(1)).unwrap();
        let sum = read.build_add(field, one).unwrap();
        read.build_ret(Some(sum)).unwrap();
        let read_id = read.func.id;
        let read = read.build();

        if reverse {
            module.add_function(read);
            module.add_function(make_two);
            module.add_function(make_one);
        } else {
            module.add_function(make_one);
            module.add_function(make_two);
            module.add_function(read);
        }

        let verified = verify_module_with_types(&module.module).unwrap();
        let types = verified.functions.get(&read_id).unwrap();
        assert_eq!(types.get(&field), Some(&TirType::Int64));
        assert_eq!(types.get(&sum), Some(&TirType::Int64));
    }
}

#[test]
fn disjoint_struct_field_sets_are_merged_across_constructions() {
    let mut module = ModuleBuilder::new();

    let mut make_a = module.new_function("make_a", TirType::Struct("S".into()));
    let entry = make_a.new_block();
    make_a.set_current_block(entry);
    let a_value = make_a.build_const(TirConst::Int64(1)).unwrap();
    let with_a = make_a
        .build_make_struct("S".into(), vec![("a".into(), a_value)])
        .unwrap();
    make_a.build_ret(Some(with_a)).unwrap();

    let mut make_b = module.new_function("make_b", TirType::Struct("S".into()));
    let entry = make_b.new_block();
    make_b.set_current_block(entry);
    let b_value = make_b.build_const(TirConst::Int64(2)).unwrap();
    let with_b = make_b
        .build_make_struct("S".into(), vec![("b".into(), b_value)])
        .unwrap();
    make_b.build_ret(Some(with_b)).unwrap();

    let mut read = module.new_function("read", TirType::Int64);
    let value = read.add_param(TirType::Struct("S".into()));
    let entry = read.new_block();
    read.set_current_block(entry);
    let a = read.build_get(value, TirKey::Field("a".into())).unwrap();
    let b = read.build_get(value, TirKey::Field("b".into())).unwrap();
    let sum = read.build_add(a, b).unwrap();
    read.build_ret(Some(sum)).unwrap();
    let read_id = read.func.id;

    module.add_function(make_a.build());
    module.add_function(make_b.build());
    module.add_function(read.build());

    let verified = verify_module_with_types(&module.module).unwrap();
    let types = verified.functions.get(&read_id).unwrap();
    assert_eq!(types.get(&a), Some(&TirType::Int64));
    assert_eq!(types.get(&b), Some(&TirType::Int64));
}

#[test]
fn cross_function_struct_field_access_is_typed() {
    for accessor_first in [false, true] {
        let mut module = ModuleBuilder::new();

        let mut make = module.new_function("make", TirType::Struct("Counter".into()));
        let entry = make.new_block();
        make.set_current_block(entry);
        let value = make.build_const(TirConst::Int64(7)).unwrap();
        let counter = make
            .build_make_struct("Counter".into(), vec![("value".into(), value)])
            .unwrap();
        make.build_ret(Some(counter)).unwrap();
        let make = make.build();

        let mut read = module.new_function("read", TirType::Int64);
        let counter = read.add_param(TirType::Struct("Counter".into()));
        let entry = read.new_block();
        read.set_current_block(entry);
        let field = read
            .build_get(counter, TirKey::Field("value".into()))
            .unwrap();
        let one = read.build_const(TirConst::Int64(1)).unwrap();
        let sum = read.build_add(field, one).unwrap();
        read.build_ret(Some(sum)).unwrap();
        let read_id = read.func.id;
        let read = read.build();

        if accessor_first {
            module.add_function(read);
            module.add_function(make);
        } else {
            module.add_function(make);
            module.add_function(read);
        }

        let verified = verify_module_with_types(&module.module).unwrap();
        assert_eq!(
            verified.functions.get(&read_id).unwrap().get(&field),
            Some(&TirType::Int64),
            "accessor_first={accessor_first}"
        );
    }
}

#[test]
fn internal_iterator_structs_remain_polymorphic() {
    let mut module = ModuleBuilder::new();
    let mut function = module.new_function("main", TirType::Unit);
    let entry = function.new_block();
    function.set_current_block(entry);

    let int_array = function
        .build_make(TirType::Array(Box::new(TirType::Int64)), None, vec![])
        .unwrap();
    let string_array = function
        .build_make(TirType::Array(Box::new(TirType::String)), None, vec![])
        .unwrap();
    let first_index = function.build_const(TirConst::Int64(0)).unwrap();
    let second_index = function.build_const(TirConst::Int64(0)).unwrap();

    let first_iter = function
        .build_make_struct(
            "$ArrayIter".into(),
            vec![("data".into(), int_array), ("idx".into(), first_index)],
        )
        .unwrap();
    function
        .build_make_struct(
            "$ArrayIter".into(),
            vec![("data".into(), string_array), ("idx".into(), second_index)],
        )
        .unwrap();

    let int_slice = function
        .build_make(TirType::Slice(Box::new(TirType::Int64)), None, vec![])
        .unwrap();
    let string_slice = function
        .build_make(TirType::Slice(Box::new(TirType::String)), None, vec![])
        .unwrap();
    let third_index = function.build_const(TirConst::Int64(0)).unwrap();
    let fourth_index = function.build_const(TirConst::Int64(0)).unwrap();

    function
        .build_make_struct(
            "$SliceIter".into(),
            vec![("data".into(), int_slice), ("idx".into(), third_index)],
        )
        .unwrap();
    function
        .build_make_struct(
            "$SliceIter".into(),
            vec![("data".into(), string_slice), ("idx".into(), fourth_index)],
        )
        .unwrap();

    let index = function
        .build_get(first_iter, TirKey::Field("idx".into()))
        .unwrap();
    let sum = function.build_add(index, second_index).unwrap();
    let _ = sum;

    function.build_ret(None).unwrap();
    module.add_function(function.build());

    let errors = match verify_module(&module.module) {
        Ok(()) => Vec::new(),
        Err(errors) => errors,
    };
    assert!(
        conflict_pairs(&errors).is_empty(),
        "internal iterator structs must not produce nominal field conflicts: {errors:?}"
    );
    assert!(errors.is_empty(), "iterator module must verify: {errors:?}");
}

fn two_conflict_module(reverse: bool) -> TirModule {
    let mut module = ModuleBuilder::new();
    let mut function = module.new_function("main", TirType::Unit);
    let entry = function.new_block();
    function.set_current_block(entry);

    let mut values = vec![
        ("Counter", "value", TirConst::Int64(1)),
        ("Counter", "value", TirConst::Float64(1.5)),
        ("Other", "count", TirConst::Bool(true)),
        ("Other", "count", TirConst::Int64(2)),
    ];
    if reverse {
        values.reverse();
    }
    for (struct_name, field, value) in values {
        let value = function.build_const(value).unwrap();
        function
            .build_make_struct(struct_name.into(), vec![(field.into(), value)])
            .unwrap();
    }

    function.build_ret(None).unwrap();
    module.add_function(function.build());
    module.module
}

#[test]
fn struct_field_conflicts_are_deterministic() {
    let expected = vec![
        expected_conflict("Counter", "value"),
        expected_conflict("Other", "count"),
    ];
    for reverse in [false, true] {
        let module = two_conflict_module(reverse);
        let first = verify_module(&module).unwrap_err();
        let second = verify_module(&module).unwrap_err();
        assert_eq!(conflict_pairs(&first), expected, "reverse={reverse}");
        assert_eq!(
            conflict_pairs(&first),
            conflict_pairs(&second),
            "reverse={reverse}"
        );
    }
}

#[test]
fn cross_struct_field_dependency_converges() {
    let mut module = ModuleBuilder::new();

    // Deliberately place the dependent construction first so the fixed point
    // must propagate B.y into A.x over multiple passes.
    let mut make_a = module.new_function("make_a", TirType::Struct("A".into()));
    let b = make_a.add_param(TirType::Struct("B".into()));
    let entry = make_a.new_block();
    make_a.set_current_block(entry);
    let y = make_a.build_get(b, TirKey::Field("y".into())).unwrap();
    let a = make_a
        .build_make_struct("A".into(), vec![("x".into(), y)])
        .unwrap();
    make_a.build_ret(Some(a)).unwrap();

    let mut make_b = module.new_function("make_b", TirType::Struct("B".into()));
    let entry = make_b.new_block();
    make_b.set_current_block(entry);
    let one = make_b.build_const(TirConst::Int64(1)).unwrap();
    let b = make_b
        .build_make_struct("B".into(), vec![("y".into(), one)])
        .unwrap();
    make_b.build_ret(Some(b)).unwrap();

    let mut read = module.new_function("read", TirType::Int64);
    let a = read.add_param(TirType::Struct("A".into()));
    let entry = read.new_block();
    read.set_current_block(entry);
    let x = read.build_get(a, TirKey::Field("x".into())).unwrap();
    let one = read.build_const(TirConst::Int64(1)).unwrap();
    let sum = read.build_add(x, one).unwrap();
    read.build_ret(Some(sum)).unwrap();
    let read_id = read.func.id;

    module.add_function(make_a.build());
    module.add_function(make_b.build());
    module.add_function(read.build());

    let verified = verify_module_with_types(&module.module).unwrap();
    assert_eq!(
        verified.functions.get(&read_id).unwrap().get(&x),
        Some(&TirType::Int64)
    );
}

fn chained_struct_module(depth: usize) -> TirModule {
    let mut module = ModuleBuilder::new();

    for index in 0..depth {
        let struct_name = format!("S{index}");
        let mut function = module.new_function(
            &format!("make_{index}"),
            TirType::Struct(struct_name.clone()),
        );
        let entry = function.new_block();
        function.set_current_block(entry);

        let value = if index + 1 < depth {
            let next = function.add_param(TirType::Struct(format!("S{}", index + 1)));
            let field = function.build_get(next, TirKey::Field("x".into())).unwrap();
            function
                .build_make_struct(struct_name, vec![("x".into(), field)])
                .unwrap()
        } else {
            let one = function.build_const(TirConst::Int64(1)).unwrap();
            function
                .build_make_struct(struct_name, vec![("x".into(), one)])
                .unwrap()
        };
        function.build_ret(Some(value)).unwrap();
        module.add_function(function.build());
    }

    let mut read = module.new_function("read", TirType::Int64);
    let root = read.add_param(TirType::Struct("S0".into()));
    let entry = read.new_block();
    read.set_current_block(entry);
    let x = read.build_get(root, TirKey::Field("x".into())).unwrap();
    let one = read.build_const(TirConst::Int64(1)).unwrap();
    let sum = read.build_add(x, one).unwrap();
    read.build_ret(Some(sum)).unwrap();
    module.add_function(read.build());

    module.module
}

#[test]
fn deep_struct_dependency_chain_converges_within_bound() {
    // One struct dependency level is propagated per pass, so a short chain
    // proves the fixed point still converges after the merge change.
    let module = chained_struct_module(8);
    verify_module_with_types(&module).unwrap();
}

#[test]
fn struct_field_inference_guard_bounds_deep_dependency_chains() {
    // The 128-pass bound is a deliberate defensive limit. A chain longer than
    // the bound must still fail with the existing non-convergence error rather
    // than looping or accepting a partially inferred layout.
    let module = chained_struct_module(130);
    let errors = verify_module(&module).unwrap_err();
    assert!(
        matches!(
            errors.as_slice(),
            [VerifierError::GenericError(message)] if message.contains("did not converge")
        ),
        "expected bounded-inference error, got {errors:?}"
    );
}
