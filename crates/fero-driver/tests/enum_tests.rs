use fero_driver::pipeline::test_source;

#[test]
fn test_enum_construction_success() {
    let source = "
    enum Status {
        Idle,
        Active(String),
    }

    fn main() {
        let s1 = Idle;
        let s2 = Active(\"running\");
        expect(s1 is Status)
        expect(s2 is Status)
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "Construction success failed: {:?}", result.err());
}

#[test]
fn test_enum_ambiguity_nullary() {
    let source = "
    enum A { Missing }
    enum B { Missing }

    fn main() {
        let x = Missing;
    }
    ";
    let result = test_source(source);
    // Should fail with ambiguity error
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("ambiguous constructor 'Missing'"));
}

#[test]
fn test_enum_ambiguity_payload() {
    let source = "
    enum A { Present(String) }
    enum B { Present(String) }

    fn main() {
        let x = Present(\"hi\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("ambiguous constructor 'Present'"));
}

#[test]
fn test_enum_expected_type_disambiguation() {
    let source = "
    enum A { Missing }
    enum B { Missing }

    fn get_a() -> A {
        Missing
    }

    fn main() {
        let a = get_a();
        expect(a is A)
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "Expected type disambiguation failed: {:?}", result.err());
}

#[test]
fn test_enum_wrong_arity() {
    let source = "
    enum MaybeName {
        Missing,
        Present(String),
    }

    fn main() {
        let x = Missing(\"wrong\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("expects 0 argument(s), found 1"));
}

#[test]
fn test_enum_lexical_shadowing_interpreter() {
    let source = "
    enum A { Missing }

    fn main() {
        let Missing = \"shadow\";
        let x = Missing;
        expect(x == \"shadow\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "Lexical shadowing failed in interpreter: {:?}", result.err());
}

#[test]
fn test_enum_nested_match_success() {
    let source = "
    enum Inner {
        Data(Int),
        Empty,
    }

    enum Outer {
        Nested(Inner),
        Other,
    }

    fn main() {
        let x = Nested(Data(42));
        let res = match x {
            Nested(Data(v)) => v,
            Nested(Empty) => 0,
            Other => -1,
        };
        expect(res == 42);
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "Nested match success failed: {:?}", result.err());
}

#[test]
fn test_enum_nested_mismatch_behavior() {
    let source = "
    enum Inner {
        Data(Int),
        Empty,
    }

    enum Outer {
        Nested(Inner),
        Other,
    }

    fn main() {
        let x = Nested(Empty);
        let matched = false;
        let res = match x {
            Nested(Data(v)) => { 
                matched = true;
                v 
            },
            _ => {
                matched = false;
                -1
            },
        };
        expect(res == -1);
        expect(matched == false);
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "Nested mismatch behavior failed: {:?}", result.err());
}

#[test]
fn test_enum_match_wrong_enum_variant() {
    let source = "
    enum A { One }
    enum B { Two }

    fn main() {
        let x = One;
        match x {
            Two => 0,
            One => 1,
        };
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("does not belong to") || err.contains("does not have variant"));
}

#[test]
fn test_enum_is_nested_success() {
    let source = "
    enum Inner { Data(Int) }
    enum Outer { Nested(Inner) }

    fn main() {
        let x = Nested(Data(1));
        if x is Nested(Data(v)) {
            expect(v == 1);
        } else {
            crash(\"should match\");
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "is nested success failed: {:?}", result.err());
}

#[test]
fn test_enum_ambiguity_if_expr_fail() {
    let source = "
    enum A { X }
    enum B { X }

    fn main() {
        let cond = true;
        let x = if cond {
            X
        } else {
            X
        };
    }
    ";
    let result = test_source(source);
    assert!(result.is_err(), "Ambiguous constructor in if-expr should fail without expected type");
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("ambiguous constructor 'X'"));
}

#[test]
fn test_enum_ambiguity_if_expr_success_with_expected() {
    let source = "
    enum A { X }
    enum B { X }

    fn get_a(cond: Bool) -> A {
        if cond {
            X
        } else {
            X
        }
    }

    fn main() {
        let a = get_a(true);
        expect(a is A);
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "Ambiguous constructor in if-expr should succeed with expected type: {:?}", result.err());
}
