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
