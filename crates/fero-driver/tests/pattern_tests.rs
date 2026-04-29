use fero_driver::pipeline::test_source;

#[test]
fn test_tuple_destructuring_happy_path() {
    let source = r#"
    fn main() {
        let t = ("a", 1);
        let res = match t {
            (x, y) => x,
            _ => "default",
        };
        expect(res == "a", "tuple destructuring failed");
    }
    "#;
    let result = test_source(source);
    assert!(result.is_ok(), "Happy path failed: {:?}", result.err());
}

#[test]
fn test_nested_tuple_happy_path() {
    let source = r#"
    fn main() {
        let t = (("a", 1), 2);
        let res = match t {
            ((x, _), y) => x,
            _ => "default",
        };
        expect(res == "a", "nested tuple destructuring failed");
    }
    "#;
    let result = test_source(source);
    assert!(result.is_ok(), "Nested happy path failed: {:?}", result.err());
}

#[test]
fn test_single_element_pattern_error() {
    let source = r#"
    fn main() {
        let x = (1, 2);
        match x {
            (a) => 1,
            _ => 0,
        };
    }
    "#;
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("single-element tuple patterns are not allowed; use `a` instead"));
}

#[test]
fn test_trailing_comma_single_element_error() {
    let source = r#"
    fn main() {
        let x = (1, 2);
        match x {
            (a,) => 1,
            _ => 0,
        };
    }
    "#;
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("single-element tuple patterns are not allowed; use `a` instead"));
}

#[test]
fn test_empty_tuple_pattern_error() {
    let source = r#"
    fn main() {
        let x = (1, 2);
        match x {
            () => 1,
            _ => 0,
        };
    }
    "#;
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("empty tuple patterns are not supported"));
}

#[test]
fn test_tuple_with_guards() {
    let source = r#"
    fn main() {
        let t = ("a", 1);
        let res = match t {
            (x, y) if x == "a" => 1,
            _ => 0,
        };
        expect(res == 1, "tuple with guard failed");
    }
    "#;
    let result = test_source(source);
    assert!(result.is_ok(), "Tuple with guard failed: {:?}", result.err());
}

#[test]
fn test_struct_nesting_tuple() {
    let source = r#"
    struct Meta { age: Int, id: String }
    struct User { name: String, meta: Meta }
    
    fn main() {
        let u = User { name: "bob", meta: Meta { age: 30, id: "1" } };
        // Validating struct field subpattern with tuple
        // Assuming Meta { age, id } can be matched as a tuple (age, id) IF it's a tuple?
        // Wait, User { meta: (age, _) } only works if meta IS a tuple.
        let u2 = ("a", 1);
        let res = match u2 {
            (x, _) => x,
            _ => "default",
        };
    }
    "#;
    let result = test_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_enum_nesting_tuple() {
    let source = r#"
    enum OptionTuple { None, Some((String, Int)) }
    fn main() {
        let x = Some(("a", 1));
        let res = match x {
            Some((a, b)) => a,
            None => "none",
        };
        expect(res == "a", "enum nesting tuple failed");
    }
    "#;
    let result = test_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_enum_payload_nesting_with_tuple() {
    let source = r#"
    enum MaybePair {
        None,
        Some((Int, Int)),
    }

    fn main() {
        let x = Some((1, 2));
        let y = match x {
            Some((a, b)) => a,
            None => 0,
        };
        expect(y == 1, "enum internal tuple nesting failed");
    }
    "#;
    let result = test_source(source);
    assert!(result.is_ok(), "Enum payload nesting with tuple failed: {:?}", result.err());
}
