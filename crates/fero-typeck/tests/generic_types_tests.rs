use fero_lexer::tokenize;
use fero_parser::Parser;
use fero_hir::lower_file;
use fero_typeck::context::TypeContext;

fn check_program(source: &str) -> Vec<String> {
    let tokens = tokenize(source);
    let mut parser = Parser::new(source, tokens);
    let file = parser.parse_file().unwrap();
    
    let mut hir_file = lower_file(source, &file);
    
    let mut ctx = TypeContext::new();
    fero_typeck::check_item::register_items(&mut ctx, &hir_file.items);
    
    for item in &mut hir_file.items {
        fero_typeck::check_item::check_item(&mut ctx, item);
    }
    
    ctx.errors.iter().map(|e| e.message.clone()).collect()
}

#[test]
fn test_generic_struct_success() {
    let source = r#"
        struct Box(type T) {
            value: T
        }
        fn main() {
            let b: Box(Int) = Box(Int) { value: 1 }
        }
    "#;
    let errors = check_program(source);
    assert!(errors.is_empty(), "Expected no errors, found: {:?}", errors);
}

#[test]
fn test_generic_struct_mismatch() {
    let source = r#"
        struct Box(type T) {
            value: T
        }
        fn main() {
            let b: Box(Int) = Box(Int) { value: "x" }
        }
    "#;
    let errors = check_program(source);
    assert!(!errors.is_empty(), "Expected type mismatch error");
    assert!(errors[0].contains("expected Int, found String"));
}

#[test]
fn test_generic_enum_success() {
    let source = r#"
        enum Maybe(type T) {
            None,
            Some(T),
        }
        fn main() {
            let x: Maybe(Int) = Some(1)
        }
    "#;
    let errors = check_program(source);
    assert!(errors.is_empty(), "Expected no errors, found: {:?}", errors);
}

#[test]
fn test_generic_enum_mismatch() {
    let source = r#"
        enum Maybe(type T) {
            None,
            Some(T),
        }
        fn main() {
            let x: Maybe(Int) = Some("x")
        }
    "#;
    let errors = check_program(source);
    assert!(!errors.is_empty(), "Expected type mismatch error");
    assert!(errors[0].contains("expected Int, found String"));
}

#[test]
fn test_nested_generic_types() {
    let source = r#"
        struct Box(type T) { value: T }
        struct Pair(type A, type B) { first: A, second: B }
        
        fn main() {
            let p: Pair(Int, Box(String)) = Pair(Int, Box(String)) {
                first: 1,
                second: Box(String) { value: "nested" }
            }
        }
    "#;
    let errors = check_program(source);
    assert!(errors.is_empty(), "Expected no errors, found: {:?}", errors);
}

#[test]
fn test_generic_enum_nominal_collison() {
    let source = r#"
        enum A(type T) { Some(T) }
        enum B(type T) { Some(T) }
        
        fn main() {
            let x: A(Int) = Some(1)
        }
    "#;
    let errors = check_program(source);
    // Since expected is A(Int), it should resolve to A::Some
    assert!(errors.is_empty(), "Expected successful resolution through expected type");
}

#[test]
fn test_generic_enum_ambiguity() {
    let source = r#"
        enum A(type T) { Some(T) }
        enum B(type T) { Some(T) }
        
        fn main() {
            let x = Some(1)
        }
    "#;
    let errors = check_program(source);
    assert!(!errors.is_empty(), "Expected ambiguity error");
    assert!(errors[0].contains("ambiguous constructor 'Some'"));
}

#[test]
fn test_generic_enum_no_expected_type() {
    let source = r#"
        enum Maybe(type T) { None, Some(T) }
        fn main() {
            let x = Some(1)
        }
    "#;
    let errors = check_program(source);
    assert!(!errors.is_empty(), "Expected error for missing expected type");
    assert!(errors.iter().any(|e| e.contains("cannot infer generic enum type")));
}

#[test]
fn test_generic_enum_nullary_success() {
    let source = r#"
        enum Maybe(type T) { None, Some(T) }
        fn main() {
            let x: Maybe(Int) = None
        }
    "#;
    let errors = check_program(source);
    assert!(errors.is_empty(), "Expected successful nullary resolution");
}

#[test]
fn test_generic_enum_nullary_no_expected_type() {
    let source = r#"
        enum Maybe(type T) { None, Some(T) }
        fn main() {
            let x = None
        }
    "#;
    let errors = check_program(source);
    assert!(!errors.is_empty(), "Expected error for missing expected type for nullary");
    assert!(errors.iter().any(|e| e.contains("cannot infer generic enum type")));
}

#[test]
fn test_generic_struct_literal_no_args() {
    let source = r#"
        struct Box(type T) { value: T }
        fn main() {
            let b = Box { value: 1 }
        }
    "#;
    let errors = check_program(source);
    assert!(!errors.is_empty(), "Expected error for missing type arguments on struct literal");
    assert!(errors.iter().any(|e| e.contains("requires 1 type argument")));
}

#[test]
fn test_generic_struct_literal_arity_mismatch() {
    let source = r#"
        struct Box(type T) { value: T }
        fn main() {
            let b = Box(Int, String) { value: 1 }
        }
    "#;
    let errors = check_program(source);
    assert!(!errors.is_empty(), "Expected error for type argument arity mismatch");
    assert!(errors.iter().any(|e| e.contains("requires 1 type argument")));
}

#[test]
fn test_generic_struct_nested_fail() {
    let source = r#"
        struct Box(type T) { value: T }
        struct Pair(type A, type B) { first: A, second: B }
        fn main() {
            let p: Pair(Int, Box(String)) = Pair(Int, Box(String)) {
                first: 1,
                second: Box(String) { value: 42 }
            }
        }
    "#;
    let errors = check_program(source);
    assert!(!errors.is_empty(), "Expected nested type mismatch");
    assert!(errors.iter().any(|e| e.contains("expected String, found Int")));
}

#[test]
fn test_generic_enum_pattern_success() {
    let source = r#"
        enum Maybe(type T) { None, Some(T) }
        fn unwrap_or(type T, value: Maybe(T), fallback: T) -> T {
            match value {
                Some(v) => v,
                None => fallback,
            }
        }
        fn main() {
            let x: Maybe(Int) = Some(1)
            let y = unwrap_or[Int](x, 0)
        }
    "#;
    let errors = check_program(source);
    assert!(errors.is_empty(), "Expected no errors in generic pattern matching, found: {:?}", errors);
}

#[test]
fn test_generic_enum_pattern_mismatch() {
    let source = r#"
        enum Maybe(type T) { None, Some(T) }
        fn unwrap_or(type T, value: Maybe(T), fallback: T) -> T {
            match value {
                Some(v) => v,
                None => fallback,
            }
        }
        fn main() {
            let x: Maybe(Int) = Some(1)
            let y = unwrap_or[Int](x, "not an int")
        }
    "#;
    let errors = check_program(source);
    assert!(!errors.is_empty(), "Expected type mismatch in generic function call");
    assert!(errors.iter().any(|e| e.contains("expected Int, found String")));
}

#[test]
fn test_generic_enum_is_narrowing() {
    let source = r#"
        enum Maybe(type T) { None, Some(T) }
        fn main() {
            let x: Maybe(Int) = Some(1)
            if x is Some(v) {
                let y: Int = v
            }
        }
    "#;
    let errors = check_program(source);
    assert!(errors.is_empty(), "Expected no errors in generic 'is' narrowing, found: {:?}", errors);
}

#[test]
fn test_generic_enum_exhaustiveness() {
    let source = r#"
        enum Maybe(type T) { None, Some(T) }
        fn main() {
            let x: Maybe(Int) = Some(1)
            let y = match x {
                Some(v) => v,
                None => 0,
            }
            
            let s: Maybe(String) = None
            let z = match s {
                Some(v) => v,
                None => "empty",
            }
        }
    "#;
    let errors = check_program(source);
    assert!(errors.is_empty(), "Expected no errors in generic exhaustiveness, found: {:?}", errors);
}
