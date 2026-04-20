use fero_driver::pipeline::test_source;

#[test]
fn test_builtin_type_resolution() {
    let source = "
    fn main() {
        let x = 1;
        expect(x is Int);
        let s = \"ok\";
        expect(s is String);
        let b = true;
        expect(b is Bool);
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "Built-in type resolution failed: {:?}", result.err());
}

#[test]
fn test_struct_type_resolution() {
    let source = "
    struct User { name: String }
    fn main() {
        let u = User { name: \"A\" };
        expect(u is User);
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "Struct type resolution failed: {:?}", result.err());
}

#[test]
fn test_enum_type_resolution() {
    let source = "
    enum MaybeName {
        Missing,
        Present(String),
    }

    fn main() {
        let x = Missing;
        expect(x is MaybeName);
        
        let y = Present(\"hi\");
        expect(y is MaybeName);
        
        // Also check pattern still works
        expect(x is Missing);
        expect(y is Present(v));
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "Enum type resolution failed: {:?}", result.err());
}

#[test]
fn test_unknown_type_diagnostic() {
    let source = "
    fn main() {
        let x = 1;
        if x is UnknownType { }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("unknown constructor") || err.contains("unknown type"));
}

#[test]
fn test_nested_pattern_resolution() {
    let source = "
    enum Data { Value(String) }
    enum Nested { Next(Data) }

    fn main() {
        let x = Next(Value(\"fero\"));
        expect(x is Next(Value(v)));
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "Nested pattern resolution failed: {:?}", result.err());
}

#[test]
fn test_invalid_naked_binding_diagnostic() {
    let source = "
    fn main() {
        let x = 1;
        if x is y { }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("naked binding patterns are not allowed after 'is'"));
}

#[test]
fn test_invalid_tuple_struct_patterns_diagnostic() {
    let source = "
    struct User { name: String }
    fn main() {
        let x = (1, 2);
        if x is (a, b) { }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
}

#[test]
fn test_invalid_struct_pattern_diagnostic() {
    let source = "
    struct User { name: String }
    fn main() {
        let u = User { name: \"A\" };
        if u is User { name: _ } { }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
}
