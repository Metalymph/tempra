use fero_parser::parse_source;

fn assert_valid_parse(source: &str) {
    match parse_source(source) {
        Ok(_) => {}
        Err(e) => panic!("Failed to parse: {}\nError: {:?}", source, e),
    }
}

#[test]
fn test_basic_statements() {
    assert_valid_parse("fn foo() { let x = 1; }");
    assert_valid_parse("fn foo() { var y = 2; }");
    assert_valid_parse("fn foo() { return x; }");
}

#[test]
fn test_functions() {
    assert_valid_parse(
        r#"
        fn add(a: UInt64, b: UInt64) -> UInt64 {
            a + b
        }
    "#,
    );
}

#[test]
fn test_if_expressions() {
    assert_valid_parse(
        r#"
        fn foo() {
            if ok {
                a
            } else {
                b
            }
        }
    "#,
    );
}

#[test]
fn test_for() {
    assert_valid_parse(
        r#"
        fn foo() {
            for x in xs {
                x
            }
            for x in xs if x > 0 {
                x
            }
        }
    "#,
    );
}

#[test]
fn test_binary_precedence() {
    assert_valid_parse("fn foo() { 1 + 2 * 3; }");
    assert_valid_parse("fn foo() { (a + b) * c; }");
    assert_valid_parse("fn foo() { a && b || c; }");
    assert_valid_parse("fn foo() { a || b && c; }");
    assert_valid_parse("fn foo() { x is String; y is String?; z is String!IoError; }");
}

#[test]
fn test_calls() {
    assert_valid_parse("fn foo() { f(x); f(x, y); f()(x); }");
    assert_valid_parse("fn foo() { f(path=\"file.txt\"); }");
    assert_valid_parse("fn foo() { f(x, path=\"file.txt\"); }");
}

fn assert_invalid_parse(source: &str) {
    match parse_source(source) {
        Ok(_) => panic!("Expected parse failure: {}", source),
        Err(_) => {}
    }
}

#[test]
fn test_invalid_calls() {
    assert_invalid_parse("fn foo() { f(path=\"a\", \"b\"); }"); // Positional after named
    assert_invalid_parse("fn foo() { f(path=\"a\", path=\"b\"); }"); // Duplicate named
}

#[test]
fn test_member_index() {
    assert_valid_parse("fn foo() { obj.field; arr[0]; arr[1..3]; }");
}

#[test]
fn test_arrays_tuples() {
    assert_valid_parse("fn foo() { [1, 2, 3]; (a, b); }");
}

#[test]
fn test_struct_literal() {
    assert_valid_parse(
        r#"
        fn foo() {
            User {
                name: "Lorenzo",
                bank_value: 50000
            };
        }
    "#,
    );
}

#[test]
fn test_match() {
    assert_valid_parse(
        r#"
        fn foo() {
            match x {
                Ok(v) => v,
                Err(e) => e
            }
        }
    "#,
    );
}

#[test]
fn test_imports() {
    assert_valid_parse("import std/fs");
    assert_valid_parse("import ./util");
    assert_valid_parse("import pkg/json");
}

#[test]
fn test_items() {
    assert_valid_parse(
        r#"
        enum Result {
            Ok(String)
            Err(String)
        }
        
        struct User {
            name: String
            var bank_value: UInt64 = 100
        }
        
        extend User {
            fn is_rich(self) -> Bool {
                self.bank_value > 100000
            }
        }
    "#,
    );
}

#[test]
fn test_option_type_sugar_parse() {
    assert_valid_parse(
        r#"
        fn f() -> Int? {
            Some(1)
        }
    "#,
    );

    assert_valid_parse(
        r#"
        struct User {
            nickname: String?
        }
    "#,
    );

    assert_valid_parse(
        r#"
        fn load() -> String! {
            Ok("x")
        }
    "#,
    );

    assert_valid_parse(
        r#"
        fn load() -> String!IoError {
            Err(IoError {})
        }
    "#,
    );
}
