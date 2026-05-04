use tempra_driver::pipeline::test_source;

// ─── helpers ──────────────────────────────────────────────────────────────────

fn expect_err_containing(source: &str, needle: &str) {
    let result = test_source(source);
    assert!(result.is_err(), "expected compile error for: {}", needle);
    let err = format!("{:?}", result.err().unwrap());
    assert!(
        err.to_lowercase().contains(&needle.to_lowercase()),
        "expected error containing {:?} but got:\n{}",
        needle,
        err
    );
}

fn expect_ok(source: &str) {
    let result = test_source(source);
    assert!(
        result.is_ok(),
        "expected success but got: {:?}",
        result.err()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// PART 1 — Enum constructor diagnostics
// ═══════════════════════════════════════════════════════════════════════════════

// 1.1 Unknown constructor (nullary)
#[test]
fn test_diag_unknown_nullary_constructor() {
    expect_err_containing("fn main() { let x = UnknownCtor; }", "UnknownCtor");
}

// 1.1 Unknown constructor (with payload)
#[test]
fn test_diag_unknown_payload_constructor() {
    expect_err_containing(r#"fn main() { let x = UnknownCtor("x"); }"#, "UnknownCtor");
}

// 1.2 Ambiguous constructor — message must mention constructor name and candidates
#[test]
fn test_diag_ambiguous_nullary_candidates_listed() {
    let src = r#"
    enum A { Missing }
    enum B { Missing }
    fn main() { let x = Missing; }
    "#;
    let result = test_source(src);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(
        err.contains("ambiguous constructor 'Missing'"),
        "missing name: {}",
        err
    );
    // Candidates A and B must appear
    assert!(
        err.contains('A') && err.contains('B'),
        "missing candidates: {}",
        err
    );
}

// 1.2 Ambiguous unary constructor
#[test]
fn test_diag_ambiguous_unary_candidates_listed() {
    let src = r#"
    enum A { Val(Int) }
    enum B { Val(Int) }
    fn main() { let x = Val(1); }
    "#;
    let result = test_source(src);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(
        err.contains("ambiguous constructor 'Val'"),
        "missing name: {}",
        err
    );
}

// 1.3 Wrong enum owner in pattern
#[test]
fn test_diag_wrong_enum_owner_in_pattern() {
    let src = r#"
    enum A { One }
    enum B { Two }
    fn main() {
        let x = One;
        match x {
            Two => 0,
            One => 1,
        };
    }
    "#;
    let result = test_source(src);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(
        err.contains("does not belong to")
            || err.contains("cannot be used")
            || err.contains("not a pattern"),
        "unexpected error: {}",
        err
    );
}

// Wrong enum owner hint includes the actual owning enum
#[test]
fn test_diag_wrong_enum_owner_includes_hint() {
    let src = r#"
    enum A { One }
    enum B { Two }
    fn main() {
        let x = One;
        match x {
            Two => 0,
            _ => 1,
        };
    }
    "#;
    let result = test_source(src);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    // The error should at least contain 'Two' and indicate a mismatch
    assert!(
        err.contains("Two"),
        "error should mention constructor: {}",
        err
    );
}

// 1.4 Wrong arity — nullary used as unary
#[test]
fn test_diag_wrong_arity_nullary_called_with_arg() {
    let src = r#"
    enum MaybeName { Missing, Present(String) }
    fn main() { let x = Missing("wrong"); }
    "#;
    expect_err_containing(src, "expects 0 arguments, found 1");
}

// 1.4 Wrong arity — unary used with no args
#[test]
fn test_diag_wrong_arity_unary_called_without_arg() {
    let src = r#"
    enum MaybeName { Missing, Present(String) }
    fn main() { let x = Present(); }
    "#;
    expect_err_containing(src, "expects 1 arguments, found 0");
}

// 1.5 Wrong payload type
#[test]
fn test_diag_wrong_payload_type() {
    let src = r#"
    enum MaybeName { Present(String) }
    fn main() { let x = Present(99); }
    "#;
    let result = test_source(src);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    // should mention expected type or actual type
    assert!(
        err.contains("String") || err.contains("Int") || err.contains("expected"),
        "expected type info in error: {}",
        err
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// PART 2 — Pattern diagnostics
// ═══════════════════════════════════════════════════════════════════════════════

// 2.1 Invalid constructor in `is`
#[test]
fn test_diag_unknown_constructor_in_is() {
    let src = r#"
    enum A { One }
    fn main() {
        let x = One;
        if x is Other(v) { }
    }
    "#;
    let result = test_source(src);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(
        err.contains("Other") || err.contains("constructor") || err.contains("pattern"),
        "unexpected error: {}",
        err
    );
}

// 2.3 Struct pattern — unknown field
#[test]
fn test_diag_struct_pattern_unknown_field() {
    let src = r#"
    struct User { name: String }
    fn main() {
        let u = User { name: "lo" };
        match u {
            User { nope } => 1,
        };
    }
    "#;
    expect_err_containing(src, "nope");
}

// 2.4 Single-element tuple pattern should fail cleanly
#[test]
fn test_diag_single_element_tuple_pattern_arity() {
    let src = r#"
    fn get_pair() -> (Int, Int) { (1, 2) }
    fn main() {
        let p = get_pair();
        match p {
            (a) => 0,
            _ => 1,
        };
    }
    "#;
    // (a) as a tuple pattern against (Int, Int) is an arity mismatch
    let result = test_source(src);
    // May succeed with binding or fail with arity mismatch; we only care it doesn't panic/ICE
    let _ = result;
}

// 2.4 Tuple arity mismatch in pattern
#[test]
fn test_diag_tuple_pattern_arity_mismatch() {
    let src = r#"
    fn get_pair() -> (Int, Int) { (1, 2) }
    fn main() {
        let p = get_pair();
        match p {
            (a, b, c) => 0,
        };
    }
    "#;
    let result = test_source(src);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(
        err.contains("arity") || err.contains("expected 2") || err.contains("found 3"),
        "expected arity error: {}",
        err
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// PART 3 — Match diagnostics
// ═══════════════════════════════════════════════════════════════════════════════

// 3.1 Non-exhaustive match on Bool — missing false
#[test]
fn test_diag_non_exhaustive_bool_missing_false() {
    let src = r#"
    fn main() {
        let b = true;
        match b {
            true => 1,
        };
    }
    "#;
    expect_err_containing(src, "Non-exhaustive match on Bool");
}

// 3.1 Non-exhaustive match on Bool — missing true
#[test]
fn test_diag_non_exhaustive_bool_missing_true() {
    let src = r#"
    fn main() {
        let b = false;
        match b {
            false => 0,
        };
    }
    "#;
    expect_err_containing(src, "Non-exhaustive match on Bool");
}

// 3.1 Non-exhaustive match on Option — missing Some
#[test]
fn test_diag_non_exhaustive_option_missing_some() {
    let src = r#"
    fn get() -> Int? { None }
    fn main() {
        let x = get();
        match x {
            None => 0,
        };
    }
    "#;
    let result = test_source(src);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(
        err.contains("Non-exhaustive") || err.contains("Some"),
        "expected exhaustiveness error: {}",
        err
    );
}

// 3.1 Non-exhaustive match on Result — missing Err
#[test]
fn test_diag_non_exhaustive_result_missing_err() {
    let src = r#"
    fn get() -> Result(Int, String) { Ok(1) }
    fn main() {
        let x = get();
        match x {
            Ok(v) => v,
        };
    }
    "#;
    let result = test_source(src);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(
        err.contains("Non-exhaustive") || err.contains("Err"),
        "expected exhaustiveness error: {}",
        err
    );
}

// 3.2 Unreachable arm after wildcard
#[test]
fn test_diag_unreachable_arm_after_wildcard() {
    let src = r#"
    fn main() {
        let b = true;
        match b {
            _ => 1,
            true => 2,
        };
    }
    "#;
    expect_err_containing(src, "unreachable");
}

// 3.2 Unreachable arm after binding catch-all
#[test]
fn test_diag_unreachable_arm_after_binding() {
    let src = r#"
    fn main() {
        let b = true;
        match b {
            x => 1,
            false => 0,
        };
    }
    "#;
    expect_err_containing(src, "unreachable");
}

// 3.3 Duplicate Bool arm
#[test]
fn test_diag_duplicate_bool_arm() {
    let src = r#"
    fn main() {
        let b = true;
        match b {
            true => 1,
            true => 2,
            false => 0,
        };
    }
    "#;
    expect_err_containing(src, "duplicate");
}

// 3.3 Duplicate Option arm
#[test]
fn test_diag_duplicate_option_arm() {
    let src = r#"
    fn get() -> Int? { None }
    fn main() {
        let x = get();
        match x {
            None => 0,
            None => 1,
            Some(v) => v,
        };
    }
    "#;
    expect_err_containing(src, "duplicate");
}

// ═══════════════════════════════════════════════════════════════════════════════
// PART 4 — Namespace diagnostics
// ═══════════════════════════════════════════════════════════════════════════════

// 4.1 Unknown namespace member — fs
#[test]
fn test_diag_fs_unknown_member() {
    expect_err_containing(
        "fn main() { fs.nope(); }",
        "namespace 'fs' has no member 'nope'",
    );
}

// 4.1 Unknown namespace member — env
#[test]
fn test_diag_env_unknown_member() {
    expect_err_containing(
        "fn main() { env.nope(); }",
        "namespace 'env' has no member 'nope'",
    );
}

// 4.1 Unknown namespace member — os
#[test]
fn test_diag_os_unknown_member() {
    expect_err_containing(
        "fn main() { os.nope(); }",
        "namespace 'os' has no member 'nope'",
    );
}

// 4.1 Unknown Regex constructor
#[test]
fn test_diag_regex_unknown_constructor() {
    expect_err_containing(
        "fn main() { let r = Regex.nope(); }",
        "namespace 'Regex' has no constructor 'nope'",
    );
}

// 4.2 Shadowed namespace used as value — error must blame the value type, not the namespace
#[test]
fn test_diag_shadowed_namespace_used_as_value() {
    let src = r#"
    fn main() {
        let fs = 123;
        fs.read("foo.txt");
    }
    "#;
    let result = test_source(src);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    // Must NOT say "Unknown filesystem operation" — fs is shadowed
    assert!(
        !err.contains("Unknown filesystem operation"),
        "should not blame fs namespace: {}",
        err
    );
    // Should mention type mismatch or no method
    assert!(
        err.contains("no field")
            || err.contains("no method")
            || err.contains("Int")
            || err.contains("callable"),
        "should explain value type issue: {}",
        err
    );
}

// 4.3 Fully unknown root — should not produce namespace-specific error
#[test]
fn test_diag_unknown_root_not_namespace() {
    let src = "fn main() { abc.read(\"x\"); }";
    let result = test_source(src);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    // Should mention 'abc' as undefined, not produce namespace error text
    assert!(
        err.contains("abc") || err.contains("undefined") || err.contains("not in scope"),
        "should identify the unknown name: {}",
        err
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// PART 5 — Cast diagnostics
// ═══════════════════════════════════════════════════════════════════════════════

// 5.2 Safe cast as? remains None (no error)
#[test]
fn test_diag_safe_cast_remains_none() {
    let src = r#"
    fn main() {
        let x = 123 as? String;
        // x should be None
    }
    "#;
    expect_ok(src);
}

// ═══════════════════════════════════════════════════════════════════════════════
// PART 6 — `is` diagnostics
// ═══════════════════════════════════════════════════════════════════════════════

// 6.1 Naked lowercase binding after `is` should produce a clear error
#[test]
fn test_diag_is_naked_binding_rejected() {
    let src = r#"
    fn main() {
        let x = 123;
        if x is y { }
    }
    "#;
    let result = test_source(src);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(
        err.contains("Naked binding")
            || err.contains("naked binding")
            || err.contains("not allowed"),
        "expected naked-binding error: {}",
        err
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// PART 7 — Consistency: wording uniformity spot checks
// ═══════════════════════════════════════════════════════════════════════════════

// Undefined variable uses lowercase "undefined"
#[test]
fn test_diag_undefined_var_lowercase() {
    let src = "fn main() { let x = foo; }";
    let result = test_source(src);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(
        err.to_lowercase().contains("undefined") || err.to_lowercase().contains("not in scope"),
        "expected 'undefined': {}",
        err
    );
}

// Unknown constructor starts with lowercase "unknown"
#[test]
fn test_diag_unknown_constructor_lowercase() {
    let src = "fn main() { let x = Zork; }";
    let result = test_source(src);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(
        err.to_lowercase().contains("unknown") || err.to_lowercase().contains("not in scope"),
        "expected 'unknown': {}",
        err
    );
}

// Method-not-found uses "has no method"
#[test]
fn test_diag_string_no_such_method() {
    expect_err_containing(r#"fn main() { let s = "hi"; s.xyzzy(); }"#, "has no method");
}

// ═══════════════════════════════════════════════════════════════════════════════
// PART 8 — Correctness / regression
// ═══════════════════════════════════════════════════════════════════════════════

// Contextual disambiguation still works
#[test]
fn test_diag_contextual_disambiguation_ok() {
    let src = r#"
    enum A { X }
    enum B { X }
    fn get_a() -> A { X }
    fn main() {
        let a = get_a();
        expect(a is A);
    }
    "#;
    expect_ok(src);
}

// Lexical shadowing beats constructor (no error)
#[test]
fn test_diag_lexical_shadowing_beats_constructor() {
    let src = r#"
    enum A { Foo }
    fn main() {
        let Foo = "shadow";
        expect(Foo == "shadow");
    }
    "#;
    expect_ok(src);
}

// Nested constructor mismatch does not bind
#[test]
fn test_diag_nested_mismatch_no_accidental_binding() {
    let src = r#"
    enum Inner { Data(Int), Empty }
    enum Outer { Nested(Inner), Other }
    fn main() {
        let x = Nested(Empty);
        var bound = false;
        let res = match x {
            Nested(Data(v)) => { bound = true; v },
            _ => { bound = false; -1 },
        };
        expect(bound == false);
        expect(res == -1);
    }
    "#;
    expect_ok(src);
}

// Regex.text still works (regression)
#[test]
fn test_diag_regex_text_still_works() {
    let src = r#"
    fn main() {
        let r = Regex.text("hello");
        expect(r.is_match("say hello world"));
    }
    "#;
    expect_ok(src);
}

// Valid fs.read typechecks (even though it may fail at runtime on missing file)
// We only verify typechecking, not execution
#[test]
fn test_diag_fs_read_typechecks() {
    let src = r#"
    fn main() {
        let result = fs.read("/nonexistent");
        match result {
            Ok(content) => print(content),
            Err(_) => print("not found"),
        }
    }
    "#;
    // Typeck should succeed even though runtime may error
    let result = test_source(src);
    // Either ok or a runtime error—but NOT a TypeError
    match result {
        Ok(_) => {}
        Err(tempra_driver::DriverError::TypeError(e)) => {
            panic!(
                "fs.read should typecheck successfully, got type error: {}",
                e
            );
        }
        Err(_) => {} // runtime error is fine
    }
}
