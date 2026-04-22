use fero_driver::pipeline::test_source;

#[test]
fn test_match_guard_happy_path() {
    let source = "
    enum MaybeInt { None, Some(Int) }

    fn main() {
        let x = Some(20);
        var y = 0;
        
        // Use if to verify value, since no direct returns yet in match expressions for our spec tests without assigning.
        match x {
            Some(v) if v > 10 => { y = 1; }
            Some(v) => { y = 2; }
            None => { y = 0; }
        }
        expect(y == 1, \"guard should match\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "Guard test failed: {:?}", result.err());
}

#[test]
fn test_match_guard_fallthrough() {
    let source = "
    enum MaybeInt { None, Some(Int) }

    fn main() {
        let x = Some(5);
        var y = 0;
        
        match x {
            Some(v) if v > 10 => { y = 1; }
            Some(v) => { y = 2; }
            None => { y = 0; }
        }
        expect(y == 2, \"guard should fall through\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "Guard fallthrough test failed: {:?}", result.err());
}

#[test]
fn test_match_guard_diagnostic_not_bool() {
    let source = "
    enum MaybeInt { None, Some(Int) }

    fn main() {
        let x = Some(1);
        match x {
            Some(v) if v => { }
            None => { }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("match guard must be Bool"));
}

#[test]
fn test_nested_struct_tuple_destructuring() {
    let source = "
    struct User {
        name: String,
        meta: (Int, Int),
    }

    fn main() {
        let u = User { name: \"A\", meta: (42, 0) };
        var resolved_age = 0;

        match u {
            User { name, meta: (age, _) } => {
                resolved_age = age;
            }
            _ => {}
        }
        expect(resolved_age == 42, \"nested struct should destruct properly\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "Nested struct test failed: {:?}", result.err());
}

#[test]
fn test_nested_tuple() {
    let source = "
    fn main() {
        let t = ((1, 2), 3);
        var res = 0;
        match t {
            ((a, b), c) => {
                res = a;
            }
            _ => {}
        }
        expect(res == 1, \"nested tuple bound properly\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "Nested tuple test failed: {:?}", result.err());
}

#[test]
fn test_match_guard_scoping() {
    let source = "
    let v = 999;

    fn main() {
        let x = Some(5);
        var y = 0;
        match x {
            Some(v) if v == 5 => { y = 1; }
            _ => { y = 0; }
        }
        expect(y == 1, \"guard should match arm local\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_match_guard_not_exhaustive() {
    let source = "
    fn main() {
        let x = Some(5);
        match x {
            Some(v) if v > 10 => { }
            None => { }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("non-exhaustive match"));
}

#[test]
fn test_match_guard_exhaustive_catchall() {
    let source = "
    enum MaybeInt { None, Some(Int) }
    
    fn main() {
        let x = Some(5);
        var y = 0;
        match x {
            Some(v) if v > 10 => { y = 1; }
            _ => { y = 0; }
        }
        expect(y == 0, \"exhaustiveness ok\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_match_guarded_same_pattern() {
    let source = "
    enum MaybeInt { None, Some(Int) }
    
    fn main() {
        let x = Some(5);
        var y = 0;
        match x {
            Some(v) if v > 10 => { y = 1; }
            Some(v) => { y = 2; }
            None => { y = 0; }
        }
        expect(y == 2, \"same pattern ok\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_match_two_guarded_same_pattern() {
    let source = "
    enum MaybeInt { None, Some(Int) }
    
    fn main() {
        let x = Some(25);
        var y = 0;
        match x {
            Some(v) if v > 30 => { y = 1; }
            Some(v) if v > 20 => { y = 2; }
            Some(_) => { y = 3; }
            None => { y = 0; }
        }
        expect(y == 2, \"two guarded ok\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "Two guards failed: {:?}", result.err());
}

#[test]
fn test_match_guarded_wildcard_non_blocking() {
    let source = "
    fn main() {
        let b = true;
        var y = 0;
        match b {
            x if false => { y = 1; }
            true => { y = 2; }
            false => { y = 3; }
        }
        expect(y == 2, \"guarded catchall does not block\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_match_unguarded_wildcard_blocks_later_arms() {
    let source = "
    fn main() {
        let b = true;
        match b {
            x => { }
            true => { }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("unreachable match arm"), "Actual error: {}", err);
}

#[test]
fn test_nested_enum_inside_field() {
    let source = "
    enum Meta {
        Data(Int),
        Empty,
    }

    struct User {
        meta: Meta,
    }

    fn main() {
        let u = User { meta: Data(7) };
        var x = 0;
        match u {
            User { meta: Data(v) } => { x = v; }
            User { meta: Empty } => { x = 0; }
            _ => {}
        }
        expect(x == 7, \"nested ok\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_duplicate_struct_field() {
    let source = "
    struct User { name: String }

    fn main() {
        let u = User { name: \"A\" };
        match u {
            User { name, name } => { }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("duplicate field 'name'"));
}

#[test]
fn test_unsupported_grouped_pattern() {
    let source = "
    fn main() {
        let t = (1, 2);
        match t {
            (a) => { }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("single-element tuple patterns are not allowed; use `a` instead"));
}

#[test]
fn test_nested_tuple_wrong_arity() {
    let source = "
    fn main() {
        let t = (1, 2);
        match t {
            (a, b, c) => { }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("tuple pattern arity mismatch: expected 2, found 3"), "Actual: {}", err);
}

#[test]
fn test_nested_struct_wrong_field() {
    let source = "
    struct User { name: String, meta: (Int, Int) }

    fn main() {
        let u = User { name: \"A\", meta: (1, 2) };
        match u {
            User { nope } => { }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("does not have a field named 'nope'"));
}

#[test]
fn test_exhaustive_enum_match() {
    let source = "
    enum MaybeName {
        Missing,
        Present(String),
    }

    fn main() {
        let x = Missing;
        var y = 0;
        match x {
            Missing => { y = 0; }
            Present(name) => { y = 1; }
        }
        expect(y == 0, \"should pass exhaustive\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "test_exhaustive_enum_match failed: {:?}", result.err());
}


#[test]
fn test_non_exhaustive_enum_match() {
    let source = "
    enum MaybeName {
        Missing,
        Present(String),
    }

    fn main() {
        let x = Missing;
        var y = 0;
        match x {
            Missing => { y = 0; }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("non-exhaustive match on enum 'MaybeName': missing arm for variant 'Present'"), "actual: {}", err);
}

#[test]
fn test_duplicate_unguarded_variant() {
    let source = "
    enum MaybeName {
        Missing,
        Present(String),
    }

    fn main() {
        let x = Missing;
        match x {
            Missing => { }
            Missing => { }
            Present(name) => { }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("duplicate match arm for variant 'Missing'"), "Actual error: {}", err);
}


#[test]
fn test_guarded_same_variant_allowed() {
    let source = "
    enum MaybeName {
        Missing,
        Present(String),
    }

    fn main() {
        let x = Present(\"a\");
        var y = 0;
        match x {
            Present(name) if name == \"b\" => { y = 1; }
            Present(name) => { y = 2; }
            Missing => { y = 0; }
        }
        expect(y == 2, \"guarded same ok\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "Actual error: {:?}", result.err());
}


#[test]
fn test_guarded_arm_does_not_count_as_full_coverage() {
    let source = "
    enum MaybeName {
        Missing,
        Present(String),
    }

    fn main() {
        let x = Present(\"a\");
        var y = 0;
        match x {
            Present(name) if name == \"b\" => { y = 1; }
            Missing => { y = 0; }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("non-exhaustive match"), "Actual error: {}", err);
}

#[test]
fn test_unguarded_variant_followed_by_guarded_unreachable() {
    let source = "
    enum MaybeName {
        Missing,
        Present(String),
    }

    fn main() {
        let x = Present(\"a\");
        var y = 0;
        match x {
            Present(name) => { y = 1; }
            Present(name) if name == \"a\" => { y = 2; }
            Missing => { y = 0; }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("unreachable match arm"), "Actual: {}", err);
}


#[test]
fn test_guarded_catch_all_does_not_cover_everything() {
    let source = "
    enum MaybeName {
        Missing,
        Present(String),
    }

    fn main() {
        let x = Missing;
        var y = 0;
        match x {
            _ if false => { y = 1; }
            Missing => { y = 0; }
            Present(name) => { y = 2; }
        }
        expect(y == 0, \"guarded ok\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok());
}


#[test]
fn test_nested_payload_patterns_count_as_top_level_coverage() {
    let source = "
    enum Inner {
        Data(Int),
        Empty,
    }

    enum Outer {
        Missing,
        Present(Inner),
    }

    fn main() {
        let x = Present(Data(1));
        var y = 0;
        match x {
            Present(Data(v)) => { y = v; }
            Missing => { y = 0; }
        }
    }
    ";
    // Based on the recommended rule: top-level pattern counts as top-level coverage regardless of inner payload shape.
    let result = test_source(source);
    assert!(result.is_ok(), "Actual error: {:?}", result.err());
}

// ─── Field Renaming Tests ─────────────────────────────────────────────────────

#[test]
fn test_field_rename_simple() {
    let source = "
    struct User {
        name: String,
        age: Int,
    }

    fn main() {
        let u = User { name: \"A\", age: 42 };
        var x = \"\";
        match u {
            User { name: n, age } => { x = n; }
            _ => {}
        }
        expect(x == \"A\", \"field rename should bind n\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "test_field_rename_simple failed: {:?}", result.err());
}

#[test]
fn test_field_rename_mixed_nested() {
    let source = "
    struct User {
        name: String,
        meta: (Int, Int),
    }

    fn main() {
        let u = User { name: \"A\", meta: (42, 0) };
        var age = 0;
        match u {
            User { name: n, meta: (a, _) } => { age = a; }
            _ => {}
        }
        expect(age == 42, \"rename + nested tuple ok\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "test_field_rename_mixed_nested failed: {:?}", result.err());
}

#[test]
fn test_field_rename_with_enum_subpattern() {
    let source = "
    enum Meta {
        Data(Int),
        Empty,
    }

    struct User {
        meta: Meta,
    }

    fn main() {
        let u = User { meta: Data(7) };
        var x = 0;
        match u {
            User { meta: Data(v) } => { x = v; }
            User { meta: Empty } => { x = 0; }
            _ => {}
        }
        expect(x == 7, \"nested enum ok\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "test_field_rename_with_enum_subpattern failed: {:?}", result.err());
}

#[test]
fn test_field_rename_wrong_field_name() {
    let source = "
    struct User { name: String }

    fn main() {
        let u = User { name: \"A\" };
        match u {
            User { nope: x } => { }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("does not have a field named 'nope'"), "Actual: {}", err);
}

#[test]
fn test_field_rename_duplicate_field() {
    let source = "
    struct User { name: String }

    fn main() {
        let u = User { name: \"A\" };
        match u {
            User { name, name: n } => { }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("duplicate field 'name'"), "Actual: {}", err);
}

#[test]
fn test_field_rename_wrong_subpattern_shape() {
    let source = "
    struct User { meta: (Int, Int) }

    fn main() {
        let u = User { meta: (1, 2) };
        match u {
            User { meta: Some(v) } => { }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
}

// ─── Coverage Model v2 Tests ─────────────────────────────────────────────────

#[test]
fn test_mixed_known_and_unknown_patterns() {
    let source = "
    enum OptionInt { None, Some(Int) }
    fn main() {
        let x = Some(5);
        var res = 0;
        match x {
            Some(v) => { res = 1; }
            y => { res = 2; }
        }
        expect(res == 1, \"mixed known + unknown\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "test_mixed_known_and_unknown_patterns failed: {:?}", result.err());
}

#[test]
fn test_unknown_coverage_not_exhaustive() {
    let source = "
    enum OptionInt { None, Some(Int) }
    fn main() {
        let x = Some(5);
        match x {
            Some(v) if v > 0 => { }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("non-exhaustive match"), "Actual err: {}", err);
}

#[test]
fn test_partial_coverage_remains_correct() {
    let source = "
    enum OptionInt { None, Some(Int) }
    fn main() {
        let x = Some(5);
        match x {
            Some(v) => { }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("non-exhaustive match"), "Actual err: {}", err);
}

#[test]
fn test_full_enum_coverage() {
    let source = "
    enum MyEnum { A, B, C }
    fn main() {
        let x = B;
        var r = 0;
        match x {
            A => { r = 1; }
            B => { r = 2; }
            C => { r = 3; }
        }
        expect(r == 2, \"full enum coverage\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "test_full_enum_coverage failed: {:?}", result.err());
}

#[test]
fn test_catch_all_dominates() {
    let source = "
    enum MyEnum { A, B, C }
    fn main() {
        let x = A;
        var r = 0;
        match x {
            _ => { r = 1; }
        }
        expect(r == 1, \"catch_all dominates\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "test_catch_all_dominates failed: {:?}", result.err());
}

#[test]
fn test_guarded_catch_all_does_not_dominate() {
    let source = "
    enum MyEnum { A, B, C }
    fn main() {
        let x = A;
        match x {
            _ if false => { }
            A => { }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("non-exhaustive match"), "Actual err: {}", err);
}

#[test]
fn test_missing_variants_sorting() {
    let source = "
    enum MyEnum { C, A, B }
    fn main() {
        let x = A;
        match x {
            B => { }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    // Missing arms for 'A', 'C' (alphabetical)
    assert!(err.contains("missing arms for variants 'A', 'C'"), "Actual err: {}", err);
}

#[test]
fn test_bool_missing_arm_wording() {
    let source = "
    fn main() {
        let b = true;
        match b {
            true => { }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("non-exhaustive match on Bool: missing arm for false"), "Actual err: {}", err);
}

#[test]
fn test_unguarded_then_guarded_same_variant() {
    let source = "
    enum Maybe {
        None,
        Some(Int),
    }

    fn main() {
        let x = Some(1);
        match x {
            Some(v) => { }
            Some(v) if v > 0 => { }
            None => { }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("unreachable match arm"), "Actual err: {}", err);
}

#[test]
fn test_guarded_only_enum_non_exhaustive() {
    let source = "
    enum Status { Idle, Active(Int) }
    fn main() {
        let x = Active(1);
        match x {
            Active(v) if v > 0 => { }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("non-exhaustive match on enum 'Status': missing arms for variants 'Active', 'Idle'"), "Actual: {}", err);
}

#[test]
fn test_guarded_only_bool_non_exhaustive() {
    let source = "
    fn main() {
        let b = true;
        let x = match b {
            true if b => 1,
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    // Since 'true' has a guard, it doesn't count. 'false' is definitely missing.
    assert!(err.contains("non-exhaustive match on Bool: missing arm for false"), "Actual: {}", err);
}

#[test]
fn test_struct_domain_conservative() {
    let source = "
    struct User { name: String }
    fn main() {
        let u = User { name: \"A\" };
        match u {
            User { name } if name == \"A\" => { }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("non-exhaustive match: this domain requires an unguarded catch-all"), "Actual: {}", err);
}

#[test]
fn test_tuple_domain_conservative() {
    let source = "
    fn main() {
        let t = (1, 2);
        match t {
            (a, b) => { }
        }
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("non-exhaustive match: this domain requires an unguarded catch-all"), "Actual: {}", err);
}

#[test]
fn test_basic_or_pattern() {
    let source = r#"
    enum Msg { A, B, C }
    fn main() {
        let x = A;
        let res = match x {
            A | B => 1,
            C => 2,
        };
        expect(res == 1, "basic OR-pattern failed");
        
        let y = B;
        let res2 = match y {
            A | B => 1,
            C => 2,
        };
        expect(res2 == 1, "basic OR-pattern second case failed");
    }
    "#;
    let result = test_source(source);
    assert!(result.is_ok(), "Basic OR-pattern failed: {:?}", result.err());
}

#[test]
fn test_or_pattern_with_bindings() {
    let source = r#"
    enum Opt { A(Int), B(Int) }
    fn main() {
        let x = A(10);
        let res = match x {
            A(v) | B(v) => v,
        };
        expect(res == 10, "OR-pattern with bindings failed");
    }
    "#;
    let result = test_source(source);
    assert!(result.is_ok(), "OR-pattern bindings failed: {:?}", result.err());
}

#[test]
fn test_or_pattern_duplicate_error() {
    let source = r#"
    enum Msg { A, B }
    fn main() {
        let x = A;
        match x {
            A | A => 1,
            _ => 0,
        };
    }
    "#;
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("duplicate pattern in OR-pattern"));
}

#[test]
fn test_or_pattern_binding_mismatch_name() {
    let source = r#"
    enum Opt { A(Int), B(Int) }
    fn main() {
        let x = A(10);
        match x {
            A(v) | B(w) => v,
            _ => 0,
        };
    }
    "#;
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("alternatives in OR-pattern must bind the same variables with the same types"));
}

#[test]
fn test_or_pattern_binding_mismatch_missing() {
    let source = r#"
    enum Opt { A(Int), B }
    fn main() {
        let x = A(10);
        match x {
            A(v) | B => 1,
            _ => 0,
        };
    }
    "#;
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("alternatives in OR-pattern must bind the same variables with the same types"));
}

#[test]
fn test_or_pattern_wildcard_forbidden() {
    let source = r#"
    enum Msg { A, B }
    fn main() {
        let x = A;
        match x {
            A | _ => 1,
        };
    }
    "#;
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("wildcards and bindings are forbidden as direct alternatives in OR-patterns"));
}

#[test]
fn test_or_pattern_binding_forbidden_at_top() {
    let source = r#"
    enum Msg { A, B }
    fn main() {
        let x = A;
        match x {
            A | v => 1,
        };
    }
    "#;
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("wildcards and bindings are forbidden as direct alternatives in OR-patterns"));
}

#[test]
fn test_or_pattern_exhaustiveness() {
    let source = r#"
    enum Msg { A, B, C }
    fn main() {
        let x = A;
        let res = match x {
            A | B => 1,
            C => 2,
        };
    }
    "#;
    let result = test_source(source);
    assert!(result.is_ok(), "Exhaustiveness with OR failed: {:?}", result.err());
}

#[test]
fn test_or_pattern_non_exhaustive() {
    let source = r#"
    enum Msg { A, B, C }
    fn main() {
        let x = A;
        match x {
            A | B => 1,
        }; 
    }
    "#;
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("non-exhaustive match"));
}

#[test]
fn test_or_pattern_reachability() {
    let source = r#"
    enum Msg { A, B, C }
    fn main() {
        let x = A;
        match x {
            A | B => 1,
            A => 2, 
        };
    }
    "#;
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("duplicate match arm for variant 'A'"));
}

#[test]
fn test_or_pattern_nested_flattening() {
    let source = r#"
    enum Msg { A, B, C, D }
    fn main() {
        let x = A;
        let res = match x {
            A | (B | C) => 1,
            D => 2,
        };
    }
    "#;
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("single-element tuple patterns are not allowed"));
}

#[test]
fn test_or_pattern_unguarded_exhaustiveness() {
    let source = r#"
    enum Msg { A, B }
    fn main() {
        let x = A;
        let res = match x {
            A | B if 1 == 1 => 1,
            _ => 0,
        };
    }
    "#;
    let result = test_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_or_pattern_guarded_non_exhaustive() {
    let source = r#"
    enum Msg { A, B }
    fn main() {
        let x = A;
        let res = match x {
            A | B if 1 == 1 => 1,
        }; // Should be non-exhaustive
    }
    "#;
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("non-exhaustive match"));
}
