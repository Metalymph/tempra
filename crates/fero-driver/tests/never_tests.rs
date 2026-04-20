use fero_driver::pipeline::{check_source, test_source};

#[test]
fn test_never_builtin_typing() {
    let source = r#"
    fn take_int(x: Int) {}
    fn fail() -> Never {
        crash("boom")
    }

    fn main() {
        // crash returns Never, which is compatible with any type
        let x = crash("abort");
        take_int(crash("boom"));
    }
    "#;
    // Use check_source to verify type-checking pass without literal execution (which would crash)
    let result = check_source(source);
    assert!(result.is_ok(), "Never builtin typing failed: {:?}", result.err());
}

#[test]
fn test_never_match_unification() {
    let source = r#"
    enum MaybeInt { None, Some(Int) }
    fn main() {
        let x = Some(1);
        let y = match x {
            Some(v) => v,
            None => crash("missing"),
        };
        expect(y == 1, "y should be 1");
    }
    "#;
    let result = test_source(source);
    assert!(result.is_ok(), "Never match unification failed: {:?}", result.err());
}

#[test]
fn test_never_if_unification() {
    let source = r#"
    fn main() {
        let cond = true;
        let x = if cond {
            42
        } else {
            crash("boom")
        };
        expect(x == 42, "x should be 42");
    }
    "#;
    let result = test_source(source);
    assert!(result.is_ok(), "Never if unification failed: {:?}", result.err());
}

#[test]
fn test_never_all_arms() {
    let source = r#"
    fn fail_all(cond: Bool) -> Never {
        if cond {
            crash("a")
        } else {
            crash("b")
        }
    }
    fn main() {}
    "#;
    let result = check_source(source);
    assert!(result.is_ok(), "Never all arms failed: {:?}", result.err());
}

#[test]
fn test_never_func_return_invalid() {
    let source = r#"
    fn fail() -> Never {
        1
    }
    fn main() {}
    "#;
    let result = check_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("function body return type mismatch: expected Never, found Int"));
}

#[test]
fn test_never_no_masking_error() {
    let source = r#"
    fn main() {
        let cond = true;
        let x = if cond {
            "a"
        } else {
            1
        };
    }
    "#;
    let result = check_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("if branches have incompatible types: String vs Int"));
}

#[test]
fn test_is_never_always_false() {
    let source = r#"
    fn main() {
        let x = 1;
        expect(!(x is Never), "x is Never should be false");
    }
    "#;
    let result = test_source(source);
    assert!(result.is_ok(), "is Never test failed: {:?}", result.err());
}

#[test]
fn test_never_match_all_arms() {
    let source = r#"
    enum Kind { A, B }
    fn fatal(k: Kind) -> Never {
        match k {
            A => crash("A"),
            B => crash("B"),
        }
    }
    fn main() {
        let k = A;
        // This won't actually execute far because fatals crash, 
        // but we verify it type-checks and compiles.
    }
    "#;
    let result = check_source(source);
    assert!(result.is_ok(), "Never match all arms failed: {:?}", result.err());
}
