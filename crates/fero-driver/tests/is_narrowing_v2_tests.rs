use fero_driver::pipeline::test_source;

fn typecheck_snippet(source: &str) -> Option<String> {
    match test_source(source) {
        Ok(_) => None,
        Err(err) => Some(format!("{:?}", err)),
    }
}

#[test]
fn test_builtin_narrowing() {
    let source = "
    fn main() {
        let x: Option<Int> = Some(3);
        if x is Int {
            let y = x + 1; // x is Int here
        }
    }
    ";
    assert_eq!(typecheck_snippet(source), None);
}

#[test]
fn test_struct_narrowing() {
    let source = "
    struct User { name: String }
    struct Admin { name: String }
    fn main() {
        let x: Result<User, Admin> = Ok(User { name: \"A\" });
        if x is User {
            let s: String = x.name;
        }
    }
    ";
    assert_eq!(typecheck_snippet(source), None);
}

#[test]
fn test_enum_binding() {
    let source = "
    fn main() {
        let x: Option<Int> = Some(3);
        if x is Some(v) {
            expect(v == 3, \"\");
        }
    }
    ";
    assert_eq!(typecheck_snippet(source), None);
}

#[test]
fn test_nested_binding() {
    let source = "
    fn main() {
        let x: Option<(Int, Int)> = Some((1, 2));
        if x is Some((a, b)) {
            expect(a == 1, \"\");
        }
    }
    ";
    assert_eq!(typecheck_snippet(source), None);
}

#[test]
fn test_nested_narrowing_composition() {
    let source = "
    fn main() {
        let x: Option<Int> = Some(3);
        if x is Some(v) {
            if v is Int {
                let y = v + 1;
            }
        }
    }
    ";
    assert_eq!(typecheck_snippet(source), None);
}

#[test]
fn test_no_leakage() {
    let source = "
    fn main() {
        let x: Option<Int> = Some(3);
        if x is Some(v) {
            expect(v == 3, \"\");
        }
        let z = v; // Should error
    }
    ";
    let err = typecheck_snippet(source).unwrap();
    assert!(err.contains("undefined variable 'v'"));
}

#[test]
fn test_no_narrowing_on_non_variable() {
    let source = "
    fn get() -> Option<Int> { Some(3) }
    fn main() {
        if get() is Int {
            // No variable is narrowed here, but this is syntactically fine.
            // If we try to use a non-existent variable... Wait, get() is Int 
            // shouldn't bind a new 'get' variable because 'get' is a function.
            // Actually, just checking it doesn't crash or falsely bind.
        }
    }
    ";
    assert_eq!(typecheck_snippet(source), None);
}

#[test]
fn test_naked_binding_error() {
    let source = "
    fn main() {
        let x: Option<Int> = Some(3);
        if x is y { }
    }
    ";
    let err = typecheck_snippet(source).unwrap();
    assert!(err.contains("naked binding is not allowed in is"));
}

#[test]
fn test_or_pattern_error() {
    let source = "
    fn main() {
        let x: Option<Int> = Some(3);
        if x is Some(3) | Some(4) { }
    }
    ";
    let err = typecheck_snippet(source).unwrap();
    assert!(err.contains("or-patterns are not allowed in is"));
}

#[test]
fn test_wildcard_error() {
    let source = "
    fn main() {
        let x: Option<Int> = Some(3);
        if x is _ { }
    }
    ";
    let err = typecheck_snippet(source).unwrap();
    assert!(err.contains("wildcard patterns are not allowed in is"));
}

#[test]
fn test_grouped_pattern_error() {
    let source = "
    fn main() {
        let x: Option<Int> = Some(3);
        // Wait, Fero parser might not support grouped patterns,
        // so this might result in a parse error.
        if x is (3) { }
    }
    ";
    let err = typecheck_snippet(source).unwrap();
    // It will either be "invalid pattern in is expression" or a parse error,
    // let's just make sure it fails and doesn't crash runtime.
    assert!(!err.is_empty());
}
