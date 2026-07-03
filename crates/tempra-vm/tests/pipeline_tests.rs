use tempra_bytecode_lower::lower_tir_module;
use tempra_hir::{lower_file, HirItem};
use tempra_parser::parse_source;
use tempra_tir::builder::ModuleBuilder;
use tempra_tir_lower::lower::lower_function as lower_tir_function;
use tempra_typeck::check_file_with_context;
use tempra_vm::*;

fn run_source(source: &str, entry: &str, args: Vec<VmValue>) -> VmValue {
    let ast = parse_source(source).expect("Parse error");
    let mut hir = lower_file(source, &ast);
    let type_ctx = check_file_with_context(&mut hir).expect("Typecheck error");

    let mut tir_builder = ModuleBuilder::new();
    for item in &hir.items {
        match item {
            HirItem::Function(func) => {
                let tir_func = lower_tir_function(func, &type_ctx, &mut tir_builder, None, None)
                    .expect("TIR lower error");
                tir_builder.add_function(tir_func);
            }
            HirItem::Extend(extend) => {
                let (target_name, target_ty) = match &extend.target {
                    tempra_hir::HirType::Named(_, name) => {
                        let ty = if type_ctx.structs.contains_key(name) {
                            tempra_hir::Type::Struct(name.clone(), vec![])
                        } else {
                            tempra_typeck::types::resolve_named_type(name)
                        };
                        (name.clone(), ty)
                    }
                    _ => continue,
                };
                for method in &extend.methods {
                    let mut method_clone = method.clone();
                    method_clone.name = format!("{}.{}", target_name, method.name);
                    let tir_func = lower_tir_function(
                        &method_clone,
                        &type_ctx,
                        &mut tir_builder,
                        Some(target_ty.clone()),
                        None,
                    )
                    .expect("TIR lower error");
                    tir_builder.add_function(tir_func);
                }
            }
            _ => {}
        }
    }

    let bc_module = lower_tir_module(&tir_builder.module).expect("Bytecode lower error");

    run_function(&bc_module, entry, args).expect("VM execution error")
}

#[test]
fn test_pipeline_add() {
    let source = "fn add(a: Int64, b: Int64) -> Int64 { return a + b; }";
    let res = run_source(source, "add", vec![VmValue::Int64(10), VmValue::Int64(20)]);
    assert_eq!(res, VmValue::Int64(30));
}

#[test]
fn test_pipeline_if() {
    let source = "fn abs(x: Int64) -> Int64 { if x < 0 { return -x; } else { return x; } }";
    assert_eq!(
        run_source(source, "abs", vec![VmValue::Int64(10)]),
        VmValue::Int64(10)
    );
    assert_eq!(
        run_source(source, "abs", vec![VmValue::Int64(-5)]),
        VmValue::Int64(5)
    );
}

#[test]
fn test_pipeline_vars() {
    let source = "fn test(x: Int64) -> Int64 { var y = x + 1; return y * 2; }";
    assert_eq!(
        run_source(source, "test", vec![VmValue::Int64(10)]),
        VmValue::Int64(22)
    );
}
#[test]
fn test_pipeline_is_some_narrowing() {
    let source = r#"
        fn main() -> Int64 {
            let x: Int64? = Some(41)
            if x is Some(v) {
                return v + 1
            } else {
                return 0
            }
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(42));
}

#[test]
fn test_pipeline_is_none() {
    let source = r#"
        fn main() -> Int64 {
            let x: Int64? = None
            if x is None {
                return 1
            } else {
                return 0
            }
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(1));
}

#[test]
fn test_pipeline_is_ok_narrowing() {
    let source = r#"
        fn main() -> Int64 {
            let r: Int64!String = Ok(10)
            if r is Ok(v) {
                return v
            } else {
                return 0
            }
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(10));
}

#[test]
fn test_pipeline_is_err_narrowing() {
    let source = r#"
        fn main() -> Int64 {
            let r: Int64!String = Err("bad")
            if r is Err(e) {
                return 1
            } else {
                return 0
            }
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(1));
}

#[test]
fn test_pipeline_is_outside_if_no_binding() {
    let source = r#"
        fn main() -> Bool {
            let x: Int64? = Some(42)
            return x is Some
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Bool(true));
}

#[test]
fn test_pipeline_array_literal() {
    let source = "fn main() -> Int64 { let xs = [1, 2, 3]; return xs[1]; }";
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(2));
}

#[test]
fn test_pipeline_array_len_call() {
    let source = "fn main() -> Int64 { let xs = [1, 2]; return xs.len(); }";
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(2));
}

#[test]
fn test_pipeline_array_len_member() {
    let source = "fn main() -> Int64 { let xs = [1, 2, 3]; return xs.len; }";
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(3));
}

#[test]
fn test_pipeline_slice_basics() {
    let source = r#"
        fn main() -> Int64 {
            let xs = [0, 1, 2, 3, 4, 5]
            let s = xs[1..4] // [1, 2, 3]
            return s[1]      // 2
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(2));
}

#[test]
fn test_pipeline_slice_len() {
    let source = "fn main() -> Int64 { let xs = [0, 1, 2, 3, 4]; return xs[1..4].len(); }";
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(3));
}

#[test]
fn test_pipeline_nested_slice() {
    let source = r#"
        fn main() -> Int64 {
            let xs = [0, 1, 2, 3, 4, 5, 6]
            let s1 = xs[1..6] // [1, 2, 3, 4, 5]
            let s2 = s1[1..4] // [2, 3, 4]
            return s2[0]      // 2
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(2));
}

#[test]
fn test_pipeline_for_array_sum() {
    let source = r#"
        fn main() -> Int64 {
            let xs = [1, 2, 3, 4]
            var sum = 0
            for x in xs {
                sum = sum + x
            }
            return sum
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(10));
}

#[test]
fn test_pipeline_for_slice_sum() {
    let source = r#"
        fn main() -> Int64 {
            let xs = [1, 2, 3, 4, 5]
            let s = xs[1..4] // [2, 3, 4]
            var sum = 0
            for x in s {
                sum = sum + x
            }
            return sum
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(9));
}

#[test]
fn test_pipeline_for_empty() {
    let source = r#"
        fn main() -> Int64 {
            let xs: [Int64] = []
            var count = 0
            for x in xs {
                count = count + 1
            }
            return count
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(0));
}

#[test]
fn test_pipeline_nested_for() {
    let source = r#"
        fn main() -> Int64 {
            let xss = [[1, 2], [3, 4]]
            var sum = 0
            for xs in xss {
                for x in xs {
                    sum = sum + x
                }
            }
            return sum
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(10));
}
#[test]
fn test_pipeline_for_unsupported_error() {
    let source = r#"
        fn main() -> Int64 {
            let x = 123
            var sum = 0
            for i in x {
                sum = sum + i
            }
            return sum
        }
    "#;
    let res = std::panic::catch_unwind(|| run_source(source, "main", vec![]));
    assert!(res.is_err());
}

#[test]
fn test_pipeline_string_basics() {
    let source = r#"
        fn main() -> String {
            return "hello tempra"
        }
    "#;
    assert_eq!(
        run_source(source, "main", vec![]),
        VmValue::String("hello tempra".into())
    );
}

#[test]
fn test_pipeline_string_eq() {
    let source = r#"
        fn main() -> Bool {
            let s1 = "abc"
            let s2 = "abc"
            let s3 = "def"
            return s1 == s2 && s1 != s3
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Bool(true));
}

#[test]
fn test_pipeline_string_len() {
    let source = r#"
        fn main() -> Int64 {
            let s = "👋 tempra" // 👋 is 1 char, space 1, tempra 6 -> total 8
            return s.len()
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(8));
}

#[test]
fn test_pipeline_string_size() {
    let source = r#"
        fn main() -> Int64 {
            let s = "👋 tempra" // 👋 is 4 bytes, space 1, tempra 6 -> total 11
            return s.size()
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(11));
}

#[test]
fn test_pipeline_string_bytes() {
    let source = r#"
        fn main() -> UInt8 {
            let s = "ABC"
            let bs = s.bytes()
            return bs[0] // 'A' is 65
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::UInt8(65));
}

#[test]
fn test_pipeline_string_unsupported() {
    let source = r#"
        fn main() -> UInt8 {
            let s = "ABC"
            return s[0]
        }
    "#;
    let res = std::panic::catch_unwind(|| run_source(source, "main", vec![]));
    assert!(res.is_err());
}

#[test]
fn test_experimental_vm_struct_literal_field_get() {
    let source = r#"
        struct User {
            name: String
            age: Int64
        }
        fn main() -> Int64 {
            let u = User { name: "Ada", age: 42 }
            return u.age
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(42));
}

#[test]
fn test_experimental_vm_struct_multiple_fields() {
    let source = r#"
        struct Point {
            x: Int64
            y: Int64
        }
        fn main() -> Int64 {
            let p = Point { x: 10, y: 20 }
            return p.x + p.y
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(30));
}

#[test]
fn test_experimental_vm_struct_field_set_value_semantics() {
    let source = r#"
        struct Point {
            var x: Int64
            var y: Int64
        }
        fn main() -> Int64 {
            var p = Point { x: 10, y: 20 }
            p.x = 100
            return p.x + p.y
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(120));
}

#[test]
fn test_experimental_vm_method_basic() {
    let source = r#"
        struct User {
            age: Int64
        }
        extend User {
            fn get_age(self) -> Int64 {
                return self.age
            }
        }
        fn main() -> Int64 {
            let u = User { age: 42 }
            return u.get_age()
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(42));
}

#[test]
fn test_experimental_vm_method_with_args() {
    let source = r#"
        struct User {
            age: Int64
        }
        extend User {
            fn plus(self, n: Int64) -> Int64 {
                return self.age + n
            }
        }
        fn main() -> Int64 {
            let u = User { age: 42 }
            return u.plus(1)
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(43));
}

#[test]
fn test_experimental_vm_extend_method() {
    let source = r#"
        struct Point {
            x: Int64
            y: Int64
        }
        extend Point {
            fn sum(self) -> Int64 {
                return self.x + self.y
            }
        }
        fn main() -> Int64 {
            let p = Point { x: 2, y: 3 }
            return p.sum()
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(5));
}

#[test]
fn test_experimental_vm_for_expr_map() {
    let source = r#"
        fn main() -> Int64 {
            let xs = [1, 2, 3]
            let ys = for x in xs {
                x + 10
            }
            return ys[1]
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(12));
}

#[test]
fn test_experimental_vm_reduce_integer_sum() {
    let source = r#"
        fn add(a: Int, b: Int) -> Int { return a + b }
        fn main() -> Int64 {
            let xs: Array(Int) = [1, 2, 3]
            let reduced = xs.reduce(add)
            if reduced is Some(v) {
                return v
            }
            return 0
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(6));
}

#[test]
fn test_experimental_vm_reduce_single_element_skips_callback() {
    let source = r#"
        fn wrong(a: Int, b: Int) -> Int { return 999 }
        fn main() -> Int64 {
            let xs: Array(Int) = [42]
            let reduced = xs.reduce(wrong)
            if reduced is Some(v) {
                return v
            }
            return 0
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(42));
}

#[test]
fn test_experimental_vm_fold_integer_sum() {
    let source = r#"
        fn add(a: Int, b: Int) -> Int { return a + b }
        fn main() -> Int64 {
            let xs: Array(Int) = [1, 2, 3]
            return xs.fold(10, add)
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(16));
}

#[test]
fn test_experimental_vm_reductions_on_slices_and_empty() {
    let source = r#"
        fn add(a: Int, b: Int) -> Int { return a + b }
        fn main() -> Int64 {
            let xs: Array(Int) = [1, 2, 3, 4]
            let sl: Slice(Int) = xs[1..4]
            let reduced = sl.reduce(add)
            let empty: Array(Int) = []
            let empty_reduced = empty.reduce(add)
            let empty_folded = empty.fold(100, add)

            var total = empty_folded
            if reduced is Some(v) {
                total = total + v
            } else {
                total = 0
            }
            if empty_reduced is Some(v) {
                return 0
            }
            return total
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(109));
}

#[test]
fn test_experimental_vm_fold_boolean_reduction() {
    let source = r#"
        fn and_bool(a: Bool, b: Bool) -> Bool { return a && b }
        fn main() -> Bool {
            let xs: Array(Bool) = [true, true, false]
            return xs.fold(true, and_bool)
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Bool(false));
}

#[test]
fn test_experimental_vm_tuple_construction() {
    let source = r#"
        fn main() -> (Int, String) {
            let pair: (Int, String) = (1, "a")
            return pair
        }
    "#;
    assert_eq!(
        run_source(source, "main", vec![]),
        VmValue::Tuple(vec![VmValue::Int64(1), VmValue::String("a".to_string())])
    );
}

#[test]
fn test_experimental_vm_tuple_equality() {
    let source = r#"
        fn main() -> Int64 {
            if (1, 2) != (1, 2) {
                return 1
            }
            if (1, 2) == (1, 3) {
                return 2
            }
            return 0
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(0));
}

#[test]
fn test_experimental_vm_tuple_destructuring_match() {
    let source = r#"
        fn main() -> Int64 {
            let pair = (10, 20)
            let res = match pair {
                (a, b) => a + b,
                _ => 0,
            }
            return res
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(30));
}

#[test]
fn test_experimental_vm_nested_tuple_destructuring_match() {
    let source = r#"
        fn main() -> Int64 {
            let nested = ((1, 2), 3)
            let res = match nested {
                ((a, _), b) => a + b,
                _ => 0,
            }
            return res
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(4));
}

#[test]
fn test_experimental_vm_enumerate_array() {
    let source = r#"
        fn main() -> Int64 {
            let xs: Array(Int64) = [10, 20, 30]
            let pairs = xs.enumerate()
            let zero: Int64 = 0
            let first = match pairs[0] {
                (i, v) => i + v,
                _ => zero,
            }
            let third = match pairs[2] {
                (i, v) => i + v,
                _ => zero,
            }
            return first + third + pairs.len()
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(45));
}

#[test]
fn test_experimental_vm_enumerate_slice() {
    let source = r#"
        fn main() -> Int64 {
            let xs: Array(Int64) = [5, 6, 7, 8]
            let sl: Slice(Int64) = xs[1..3]
            let pairs = sl.enumerate()
            let zero: Int64 = 0
            let first = match pairs[0] {
                (i, v) => i + v,
                _ => zero,
            }
            let second = match pairs[1] {
                (i, v) => i + v,
                _ => zero,
            }
            return first + second + pairs.len()
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(16));
}

#[test]
fn test_experimental_vm_zip_array_array() {
    let source = r#"
        fn main() -> Int64 {
            let xs: Array(Int64) = [1, 2]
            let ys: Array(Int64) = [10, 20]
            let pairs = xs.zip(ys)
            let zero: Int64 = 0
            let second = match pairs[1] {
                (a, b) => a + b,
                _ => zero,
            }
            return second + pairs.len()
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(24));
}

#[test]
fn test_experimental_vm_zip_array_slice_and_slice_array() {
    let source = r#"
        fn main() -> Int64 {
            let xs: Array(Int64) = [1, 2, 3]
            let ys: Array(Int64) = [10, 20, 30]
            let xsl: Slice(Int64) = xs[0..2]
            let ysl: Slice(Int64) = ys[1..3]

            let array_slice = xs.zip(ysl)
            let slice_array = xsl.zip(ys)
            let zero: Int64 = 0
            let a = match array_slice[1] {
                (left, right) => left + right,
                _ => zero,
            }
            let b = match slice_array[1] {
                (left, right) => left + right,
                _ => zero,
            }
            return a + b + array_slice.len() + slice_array.len()
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(58));
}

#[test]
fn test_experimental_vm_zip_slice_slice() {
    let source = r#"
        fn main() -> Int64 {
            let xs: Array(Int64) = [1, 2, 3]
            let ys: Array(Int64) = [10, 20, 30]
            let xsl: Slice(Int64) = xs[1..3]
            let ysl: Slice(Int64) = ys[0..2]
            let pairs = xsl.zip(ysl)
            let zero: Int64 = 0
            let first = match pairs[0] {
                (left, right) => left + right,
                _ => zero,
            }
            return first + pairs.len()
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(14));
}

#[test]
fn test_experimental_vm_zip_stops_at_shortest_and_empty() {
    let source = r#"
        fn main() -> Int64 {
            let left_short: Array(Int) = [1]
            let right_long: Array(Int) = [10, 20, 30]
            let a = left_short.zip(right_long)

            let left_long: Array(Int) = [1, 2, 3]
            let right_short: Array(Int) = [10]
            let b = left_long.zip(right_short)

            let empty: Array(Int) = []
            let c = empty.zip(left_long)

            return a.len() + b.len() + c.len()
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(2));
}

#[test]
fn test_experimental_vm_for_expr_slice() {
    let source = r#"
        fn main() -> Int64 {
            let xs = [1, 2, 3, 4]
            let s = xs[1..3]
            let ys = for x in s {
                x * 2
            }
            return ys[1]
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(6));
}

#[test]
fn test_experimental_vm_for_expr_empty() {
    let source = r#"
        fn main() -> Int64 {
            let xs: [Int64] = []
            let ys = for x in xs {
                x + 1
            }
            return ys.len()
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(0));
}

#[test]
fn test_experimental_vm_for_expr_complex_body() {
    let source = r#"
        fn main() -> Int64 {
            let xs = [1, 2, 3]
            let ys = for x in xs {
                let y = x * 2
                y + 5
            }
            return ys[2]
        }
    "#;
    assert_eq!(run_source(source, "main", vec![]), VmValue::Int64(11));
}
