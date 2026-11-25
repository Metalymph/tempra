use tempra_vm::value::VmValue;

fn run_vm_test(source: &str, expected: VmValue) {
    let actual = tempra_driver::run_source_vm(source, &[]).unwrap();
    assert_eq!(actual, expected);
}

fn run_vm_error_test(source: &str) {
    assert!(tempra_driver::run_source_vm(source, &[]).is_err());
}

#[test]
fn vm_heap_source_string_len_ascii() {
    let source = r#"
        fn strlen(s: String) -> Int64 {
            return s.len()
        }
        fn main() -> Int64 {
            let s1 = strlen("hello")
            let s2 = strlen("hello")
            return s1 + s2
        }
    "#;
    run_vm_test(source, VmValue::Int64(10));
}

#[test]
fn vm_heap_source_string_len_unicode() {
    let source = r#"
        fn strlen(s: String) -> Int64 {
            return s.len()
        }
        fn main() -> Int64 {
            let s1 = strlen("hé")
            let s2 = strlen("hé")
            return s1 + s2
        }
    "#;
    run_vm_test(source, VmValue::Int64(4));
}

#[test]
fn vm_heap_source_string_size_unicode() {
    let source = r#"
        fn ssize(s: String) -> Int64 {
            return s.size()
        }
        fn main() -> Int64 {
            let s1 = ssize("hé")
            let s2 = ssize("hé")
            return s1 + s2
        }
    "#;
    run_vm_test(source, VmValue::Int64(6));
}

#[test]
fn vm_heap_source_array_len() {
    let source = r#"
        fn get_len(xs: [Int64]) -> Int64 {
            return xs.len()
        }
        fn main() -> Int64 {
            let xs: [Int64] = [10, 20, 30]
            let l1 = get_len(xs)
            let l2 = get_len(xs)
            return l1 + l2
        }
    "#;
    run_vm_test(source, VmValue::Int64(6));
}

#[test]
fn vm_heap_source_array_get_i64() {
    let source = r#"
        fn get(xs: [Int64]) -> Int64 {
            return xs[1]
        }
        fn main() -> Int64 {
            let xs: [Int64] = [10, 20, 30]
            let v1 = get(xs)
            let v2 = get(xs)
            return v1 + v2
        }
    "#;
    run_vm_test(source, VmValue::Int64(40));
}

#[test]
fn vm_heap_source_array_get_negative_i64() {
    let source = r#"
        fn get(xs: [Int64]) -> Int64 {
            return xs[-1]
        }
        fn main() -> Int64 {
            let xs: [Int64] = [10, 20, 30]
            let v1 = get(xs)
            return v1
        }
    "#;
    run_vm_error_test(source);
}

#[test]
fn vm_heap_source_mixed_int_and_heap() {
    let source = r#"
        fn process(s: String, xs: [Int64], base: Int64) -> Int64 {
            let l1 = s.len()
            let l2 = xs.len()
            let val = xs[0]
            return base + l1 + l2 + val
        }
        fn main() -> Int64 {
            let xs: [Int64] = [100, 200]
            let v1 = process("test", xs, 50)
            let v2 = process("test", xs, 50)
            return v1 + v2
        }
    "#;
    // base (50) + l1 (4) + l2 (2) + val (100) = 156 * 2 = 312
    run_vm_test(source, VmValue::Int64(312));
}

#[test]
fn vm_heap_source_generic_mixed() {
    let source = r#"
        fn f(s: String, xs: [Int64]) -> Int64 {
            return s.len() + s.size() + xs.len() + xs[1]
        }
        fn main() -> Int64 {
            let xs: [Int64] = [10, 20, 30]
            let v1 = f("hé", xs)
            let v2 = f("hé", xs)
            return v1 + v2
        }
    "#;
    // f("hé", [10, 20, 30]) = 2 + 3 + 3 + 20 = 28. 28 * 2 = 56.
    run_vm_test(source, VmValue::Int64(56));
}

#[test]
fn vm_heap_source_array_get_oob_error() {
    let source = r#"
        fn get(xs: [Int64]) -> Int64 {
            return xs[5]
        }
        fn main() -> Int64 {
            let xs: [Int64] = [10, 20, 30]
            return get(xs)
        }
    "#;
    run_vm_error_test(source);
}
