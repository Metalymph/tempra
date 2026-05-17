use tempra_hir::lower_file;
use tempra_parser::parse_source;
use tempra_typeck::check_file;

fn expect_ok(source: &str) {
    let ast = parse_source(source).expect("Failed to parse source");
    let mut hir = lower_file(source, &ast);
    if let Err(errors) = check_file(&mut hir) {
        panic!(
            "Type checking failed for source:\n{}\nErrors: {:?}",
            source, errors
        );
    }
}

fn expect_err(source: &str) {
    let ast = match parse_source(source) {
        Ok(ast) => ast,
        Err(_) => return, // Parse error is also a form of error
    };
    let mut hir = lower_file(source, &ast);
    if let Err(errs) = check_file(&mut hir) {
        if errs.is_empty() {
            panic!(
                "check_file returned Err but with no errors? for source:\n{}",
                source
            );
        }
        return;
    }
    panic!("Expected error, but succeeded for source:\n{}", source);
}

#[test]
fn test_as_optional_success() {
    expect_ok(
        r#"
        fn main() {
            let s = "hello" as? String;
            let n = 123 as? String;
            let o = Some("hello") as? String?;
        }
    "#,
    );
}

#[test]
fn test_generic_function_inference() {
    expect_ok(
        r#"
        fn identity(type T, value: T) -> T {
            return value
        }

        fn main() {
            let x = identity(42);
            let y = identity("hello");
            let z = identity(true);
        }
    "#,
    );
}

#[test]
fn test_generic_function_specialization() {
    expect_ok(
        r#"
        fn identity(type T, value: T) -> T {
            return value
        }

        fn main() {
            let x = identity[Int](42);
            let y = identity[String]("hello");
        }
    "#,
    );
}

#[test]
fn test_generic_function_specialization_mismatch() {
    expect_err(
        r#"
        fn identity(type T, value: T) -> T {
            return value
        }

        fn main() {
            let x = identity[String](42);
        }
    "#,
    );
}

#[test]
fn test_recursive_substitution() {
    expect_ok(
        r#"
        fn identity(type T, x: T) -> T {
            return x
        }

        fn main() {
            let a = identity[(Int, String)]((1, "x"));
        }
    "#,
    );
}

#[test]
fn test_nested_inference() {
    expect_ok(
        r#"
        fn identity(type T, value: T) -> T {
            return value
        }
        fn wrap(type T, x: T) -> T {
            return x
        }

        fn main() {
            let x = wrap(identity(1));
        }
    "#,
    );
}

#[test]
fn test_inference_conflict() {
    expect_err(
        r#"
        fn pair(type T, a: T, b: T) {}

        fn main() {
            pair(1, "x");
        }
    "#,
    );
}

#[test]
fn test_specialization_arity_mismatch() {
    expect_err(
        r#"
        fn identity(type T, value: T) -> T {
            return value
        }

        fn main() {
            let x = identity[Int, String](1);
        }
    "#,
    );
}

#[test]
fn test_semantic_identity_isolation() {
    expect_ok(
        r#"
        fn f(type T, x: T) -> T { return x }
        fn g(type T, x: T) -> T { return x }

        fn main() {
            let a = f(1);
            let b = g("s");
        }
    "#,
    );
}

#[test]
fn test_as_forced_success() {
    expect_ok(
        r#"
        fn main() {
            let s = "hello" as! String;
            let n = 123 as! Int;
            let o = Some(1) as! Int?;
        }
    "#,
    );
}

#[test]
fn test_as_plain_failure() {
    let source = r#"
        fn main() {
            let s = "hello" as String;
        }
    "#;
    assert!(parse_source(source).is_err());
}

#[test]
fn test_narrowing_primitive() {
    expect_ok(
        r#"
        fn main() {
            let x = "hello" as? String;
            if x is String {
                let l = x.len(); // Method exists on String
            }
        }
    "#,
    );
}

#[test]
fn test_narrowing_negative() {
    expect_err(
        r#"
        fn main() {
            let x = 123 as? String;
            if x is String {
            }
            let l = x.len(); // Should fail because x is Option outside if
        }
    "#,
    );
}

#[test]
fn test_narrowing_option() {
    expect_ok(
        r#"
        fn main() {
            let x = Some("hello") as? String?;
            if x is String? {
            }
        }
    "#,
    );
}

#[test]
fn test_is_pattern_some_binding() {
    expect_ok(
        r#"
        fn main() {
            let x: String? = Some("hello");
            if x is Some(v) {
                let l = v.len();
            }
        }
    "#,
    );
}

#[test]
fn test_is_pattern_err_binding() {
    expect_ok(
        r#"
        fn main() {
            if Err("fail") as! Result(Int, String) is Err(e) {
                let s = e.len();
            }
        }
    "#,
    );
}

#[test]
fn test_is_pattern_isolation() {
    expect_ok(
        r#"
        fn main() {
            let x = 1;
            if Some("inner") as! String? is Some(x) {
                let l = x.len(); // inner x is String
            }
            let a = x + 1; // outer x is Int
        }
    "#,
    );
}

#[test]
fn test_is_pattern_naked_binding_rejected() {
    expect_err(
        r#"
        fn main() {
            let value = "hello";
            if value is x {
                print(x);
            }
        }
    "#,
    );
}

#[test]
fn test_is_pattern_naked_foo_rejected() {
    expect_err(
        r#"
        fn main() {
            let value = "hello";
            if value is foo {
                print("foo");
            }
        }
    "#,
    );
}

#[test]
fn test_is_pattern_negative_leak() {
    expect_err(
        r#"
        fn main() {
            let y = Some("hello");
            if y is Some(v) {
                let l = v.len();
            }
            let l = v.len(); // v should not exist here
        }
    "#,
    );
}

#[test]
fn test_literal_typing() {
    expect_ok(
        r#"
        fn main() {
            let x = 1;
            let y = 1.5;
            let z = "hello";
            let ok = true;
        }
    "#,
    );
}

#[test]
fn test_arithmetic_success() {
    expect_ok(
        r#"
        fn main() {
            let x = 1 + 2;
        }
    "#,
    );
}

#[test]
fn test_arithmetic_failure() {
    expect_err(
        r#"
        fn main() {
            let x = "a" + 1;
        }
    "#,
    );
}

#[test]
fn test_if_success() {
    expect_ok(
        r#"
        fn main() {
            let x = if true { 1 } else { 2 };
        }
    "#,
    );
}

#[test]
fn test_if_failure() {
    expect_err(
        r#"
        fn main() {
            let x = if true { 1 } else { "a" };
        }
    "#,
    );
}

#[test]
fn test_function_call_success() {
    expect_ok(
        r#"
        fn add(a: Int, b: Int) -> Int {
            a + b
        }

        fn main() {
            let x = add(1, 2);
        }
    "#,
    );
}

#[test]
fn test_function_call_arity_failure() {
    expect_err(
        r#"
        fn add(a: Int, b: Int) -> Int {
            a + b
        }

        fn main() {
            let x = add(1);
        }
    "#,
    );
}

#[test]
fn test_struct_literal_success() {
    expect_ok(
        r#"
        struct User {
            name: String,
            bank_value: Int
        }

        fn main() {
            let u = User {
                name: "Lorenzo",
                bank_value: 50000
            };
        }
    "#,
    );
}

#[test]
fn test_struct_literal_failure() {
    expect_err(
        r#"
        struct User {
            name: String,
            bank_value: Int
        }

        fn main() {
            let u = User {
                name: 123,
                bank_value: 50000
            };
        }
    "#,
    );
}

#[test]
fn test_array_success() {
    expect_ok(
        r#"
        fn main() {
            let xs = [1, 2, 3];
        }
    "#,
    );
}

#[test]
fn test_array_failure() {
    expect_err(
        r#"
        fn main() {
            let xs = [1, "a", 3];
        }
    "#,
    );
}

#[test]
fn test_for_statement_success() {
    expect_ok(
        r#"
        fn main() {
            let xs = [1, 2, 3];
            for x in xs {
                x;
            }
        }
    "#,
    );
}

#[test]
fn test_for_expression_success() {
    expect_ok(
        r#"
        fn main() {
            let xs = [1, 2, 3];
            let ys = for x in xs {
                x + 1
            };
        }
    "#,
    );
}

#[test]
fn test_print_success() {
    expect_ok(
        r#"
        fn main() {
            print("hello")
            print("a", 1, true)
            print()
        }
    "#,
    );
}

#[test]
fn test_fmt_success() {
    expect_ok(
        r#"
        fn main() {
            let s = fmt("a", 1, true)
            let s2 = fmt()
        }
    "#,
    );
}

#[test]
fn test_read_line_success() {
    expect_ok(
        r#"
        fn main() {
            let name = read_line()
            print(name)
        }
    "#,
    );
}

#[test]
fn test_format_return_type() {
    expect_ok(
        r#"
        fn main() {
            let s = fmt("value=", 5)
            print(s)
        }
    "#,
    );
}

#[test]
fn test_read_line_wrong_arity() {
    expect_err(
        r#"
        fn main() {
            read_line(1)
        }
    "#,
    );
}

#[test]
fn test_cmd_success() {
    expect_ok(
        r#"
        fn main() {
            let c = cmd("echo", "hello");
        }
    "#,
    );
}

#[test]
fn test_cmd_wrong_arity() {
    expect_err(
        r#"
        fn main() {
            let c = cmd();
        }
    "#,
    );
}

#[test]
fn test_run_method_success() {
    expect_ok(
        r#"
        fn main() {
            let r = cmd("echo", "hello").run();
        }
    "#,
    );
}

#[test]
fn test_command_pipe_success() {
    expect_ok(
        r#"
        fn main() {
            let r = cmd("echo", "hello") | cmd("grep", "h");
        }
    "#,
    );
}

#[test]
fn test_read_success() {
    expect_ok(
        r#"
        fn main() {
            let text = fs.read("notes.txt")!;
            print(text);
        }
    "#,
    );
}

#[test]
fn test_read_wrong_type() {
    expect_err(
        r#"
        fn main() {
            let text = fs.read(123);
        }
    "#,
    );
}

#[test]
fn test_write_success() {
    expect_ok(
        r#"
        fn main() {
            fs.write("out.txt", "hello")!;
        }
    "#,
    );
}

#[test]
fn test_write_wrong_type() {
    expect_err(
        r#"
        fn main() {
            fs.write("out.txt", 123)!;
        }
    "#,
    );
}

#[test]
fn test_append_success() {
    expect_ok(
        r#"
        fn main() {
            fs.append("log.txt", "done")!;
        }
    "#,
    );
}

#[test]
fn test_append_wrong_type() {
    expect_err(
        r#"
        fn main() {
            fs.append("log.txt", 123);
        }
    "#,
    );
}

#[test]
fn test_exit_success() {
    expect_ok(
        r#"
        fn main() {
            exit(1);
        }
    "#,
    );
}

#[test]
fn test_exit_wrong_type() {
    expect_err(
        r#"
        fn main() {
            exit("1");
        }
    "#,
    );
}

#[test]
fn test_expect_success() {
    expect_ok(
        r#"
        fn main() {
            expect(true);
            expect(false, "message");
        }
    "#,
    );
}

#[test]
fn test_expect_wrong_type() {
    expect_err(
        r#"
        fn main() {
            expect(1);
        }
    "#,
    );
    expect_err(
        r#"
        fn main() {
            expect(false, 1);
        }
    "#,
    );
}

#[test]
fn test_dbg_preserves_type() {
    expect_ok(
        r#"
        fn take_int(x: Int) {}
        fn take_string(s: String) {}
        
        fn main() {
            take_int(dbg(1));
            take_string(dbg("hello"));
        }
    "#,
    );
}

#[test]
fn test_crash_success() {
    expect_ok(
        r#"
        fn main() {
            crash("boom");
        }
    "#,
    );
}

#[test]
fn test_crash_failure() {
    expect_err(
        r#"
        fn main() {
            crash(); // arity
        }
    "#,
    );
    expect_err(
        r#"
        fn main() {
            crash(123); // type
        }
    "#,
    );
}

#[test]
fn test_env_get_typeck() {
    expect_ok(
        r#"
        let home = env.get("HOME")?
        print(home)
    "#,
    );
    expect_err(
        r#"
        let home = env.get(1)?
    "#,
    );
}

#[test]
fn test_fs_exists_typeck() {
    expect_ok(
        r#"
        let ok = fs.exists("file.txt")?
        expect(ok)
    "#,
    );
    expect_err(
        r#"
        let ok = fs.exists(1)?
    "#,
    );
}

#[test]
fn test_named_arguments_builtins() {
    expect_ok(
        r#"
        fn main() {
            exit(code=1);
            crash(message="error");
            expect(condition=true, message="ok");
            expect(true, message="ok");
            let _c1 = cmd("ls", "-l", cwd=".");
            let _c2 = cmd("ls", check=true);
            print("a", "b", newline=false);
            let _s = fmt("1", "2", separator="-");
            let _x = dbg(123, label="val");
        }
    "#,
    );
}

#[test]
fn test_named_arguments_user_functions() {
    expect_ok(
        r#"
        fn foo(x: Int, y: String) {}
        fn main() {
            foo(1, "a");
            foo(x=1, y="a");
            foo(1, y="a");
            foo(y="a", x=1);
        }
    "#,
    );
}

#[test]
fn test_named_arguments_failures() {
    expect_err(
        r#"
        fn foo(x: Int) {}
        fn main() {
            foo(z=1); // Unknown named arg
        }
    "#,
    );
    expect_err(
        r#"
        fn foo(x: Int) {}
        fn main() {
            foo(1, x=2); // Duplicate mapping
        }
    "#,
    );
    expect_err(
        r#"
        fn foo(x: Int, y: Int) {}
        fn main() {
            foo(y=1); // Missing x
        }
    "#,
    );
}

#[test]
fn test_namespaced_named_arguments() {
    expect_ok(
        r#"
        fn main() {
            let r1 = fs.read(path="file.txt")!;
            let r2 = fs.write(path="out.txt", text="hi")!;
            let r3 = env.get(name="PATH")!;
        }
    "#,
    );
}
#[test]
fn test_os_stdlib_typeck() {
    expect_ok(
        r#"
        fn main() ! {
            let args = os.args();
            let cwd = os.cwd()?;
            os.chdir("..")?;
        }
    "#,
    );

    // Invalid arity
    expect_err(r#"fn main() { os.args(1); }"#);
    expect_err(r#"fn main() { os.cwd(1); }"#);
    expect_err(r#"fn main() { os.chdir(); }"#);

    // Invalid type
    expect_err(r#"fn main() { os.chdir(123); }"#);
}
#[test]
fn test_string_methods_typeck() {
    expect_ok(
        r#"
        fn main() {
            let n = "abc".len();
            let s = "  hi  ".trim();
            let s2 = "hi".trim_start();
            let s3 = "hi".trim_end();
            let ok = "hello".contains("ell");
            let ok2 = "hello".starts_with("he");
            let ok3 = "hello".ends_with("lo");
            let xs = "a,b,c".split(",");
            let s4 = "abc".replace("a", "z");
            let s5 = "ABC".to_lower();
            let s6 = "abc".to_upper();
        }
    "#,
    );

    expect_err(r#"fn main() { 1.trim(); }"#);
    expect_err(r#"fn main() { "abc".contains(1); }"#);
    expect_err(r#"fn main() { "abc".replace("x"); }"#);
}

#[test]
fn test_fs_v0_2_typeck() {
    expect_ok(
        r#"
        fn main() ! {
            fs.mkdir("tmp")?;
            let entries = fs.list_dir("examples")?;
            fs.rename("a", "b")?;
            fs.copy("a", "b")?;
        }
    "#,
    );

    expect_err(r#"fn main() { fs.mkdir(); }"#);
    expect_err(r#"fn main() { fs.list_dir(1); }"#);
    expect_err(r#"fn main() { fs.rename("a"); }"#);
    expect_err(r#"fn main() { fs.copy("a", 1); }"#);
}
#[test]
fn test_struct_defaults_success() {
    expect_ok(
        r#"
        struct User {
            name: String = "anon"
            age: Int = 0
        }
        fn main() {
            let u1 = User {}
            let u2 = User { name: "me" }
            let u3 = User { age: 10 }
            let u4 = User { name: "me", age: 10 }
        }
    "#,
    );
}

#[test]
fn test_struct_missing_required_field() {
    expect_err(
        r#"
        struct User {
            name: String
            age: Int = 0
        }
        fn main() {
            let u = User { age: 10 }
        }
    "#,
    );
}

#[test]
fn test_struct_field_mutability() {
    expect_ok(
        r#"
        struct User {
            name: String
            var age: Int = 0
        }
        fn main() {
            let u = User { name: "me" }
            u.age = 21
        }
    "#,
    );

    expect_err(
        r#"
        struct User {
            name: String
            age: Int = 0
        }
        fn main() {
            let u = User { name: "me" }
            u.name = "you" // immutable
        }
    "#,
    );
}

#[test]
fn test_struct_visibility_syntax() {
    // Visibility is mostly for cross-module, but we check syntax works in same module
    expect_ok(
        r#"
        pub struct User {
            pub name: String
            age: Int
        }
        fn main() {
            let u = User { name: "me", age: 29 }
            print(u.name)
            print(u.age)
        }
    "#,
    );
}

#[test]
fn test_string_unicode_typeck() {
    expect_ok(
        r#"
        fn main() {
            let s = "hé";
            let l = s.len();
            let sz = s.size();
            for ch in s {
                print(ch);
            }
        }
    "#,
    );
}

#[test]
fn test_slice_model_typeck() {
    expect_ok(
        r#"
        fn main() {
            let xs = [1, 2, 3, 4]
            let ys = xs[1..3]
            let l = ys.len()
            let z = ys[0]
            let zs = ys[0..1]
        }
    "#,
    );
}

#[test]
fn test_string_bytes_typeck() {
    // bytes() returns Slice[UInt8]
    expect_ok(
        r#"
        fn main() {
            let bs = "hé".bytes()
            let n = bs.len()
        }
    "#,
    );
}

#[test]
fn test_string_bytes_no_args_typeck() {
    // bytes() with arg must fail
    expect_err(
        r#"
        fn main() {
            let bs = "hello".bytes(1)
        }
    "#,
    );
}

#[test]
fn test_string_bytes_for_loop_typeck() {
    expect_ok(
        r#"
        fn main() {
            for b in "hé".bytes() {
                print(b)
            }
        }
    "#,
    );
}

#[test]
fn test_regex_constructors_typeck() {
    expect_ok(
        r#"
        fn main() {
            let a = Regex.text("abc")
            let b = Regex.any()
            let c = Regex.seq(Regex.text("a"), Regex.text("b"))
            let d = Regex.alt(Regex.text("a"), Regex.text("b"))
            let e = Regex.one_or_more(Regex.digit())
            let sink = [a, b, c, d, e]
            print(sink)
        }
    "#,
    );
}

#[test]
fn test_regex_methods_typeck() {
    expect_ok(
        r#"
        fn main() {
            let re = Regex.text("abc")
            expect(re.is_match("zabcx"))
            let out = re.replace("abc abc", "x")
            print(out)
        }
    "#,
    );
}

#[test]
fn test_regex_invalid_typeck() {
    expect_err(
        r#"
        fn main() {
            Regex.text(1)
        }
    "#,
    );

    expect_err(
        r#"
        fn main() {
            Regex.optional("x")
        }
    "#,
    );

    expect_err(
        r#"
        fn main() {
            Regex.seq()
        }
    "#,
    );

    expect_err(
        r#"
        fn main() {
            Regex.alt(1, 2)
        }
    "#,
    );

    expect_err(
        r#"
        fn main() {
            Regex.digit(1)
        }
    "#,
    );

    expect_err(
        r#"
        fn main() {
            let re = Regex.text("a")
            re.is_match(1)
        }
    "#,
    );

    expect_err(
        r#"
        fn main() {
            let re = Regex.text("a")
            re.replace("abc", 1)
        }
    "#,
    );
}

#[test]
fn test_option_optional_return_syntax_typeck() {
    expect_ok(
        r#"
        fn f() -> Int? {
            Some(1)
        }

        fn main() {
            print(f())
        }
    "#,
    );
}

#[test]
fn test_option_none_in_context_typeck() {
    expect_ok(
        r#"
        fn f() -> Int? {
            None
        }

        fn main() {
            print(f())
        }
    "#,
    );
}

#[test]
fn test_option_some_string_typeck() {
    expect_ok(
        r#"
        fn main() {
            let x: String? = Some("hello")
            print(x)
        }
    "#,
    );
}

#[test]
fn test_option_invalid_some_return_mismatch_typeck() {
    expect_err(
        r#"
        fn f() -> Int? {
            Some("x")
        }
    "#,
    );
}

#[test]
fn test_option_ambiguous_none_typeck() {
    expect_err(
        r#"
        fn main() {
            let x = None
            print(x)
        }
    "#,
    );
}

#[test]
fn test_result_default_error_sugar_typeck() {
    expect_ok(
        r#"
        fn load() -> String! {
            Ok("x")
        }
    "#,
    );
}

#[test]
fn test_result_explicit_error_sugar_typeck() {
    expect_ok(
        r#"
        struct IoError {}

        fn load() -> String!IoError {
            Err(IoError {})
        }
    "#,
    );
}

#[test]
fn test_result_unit_fallible_sugar_typeck() {
    expect_ok(
        r#"
        fn main()! {
            print("ok")
        }
    "#,
    );
}

#[test]
fn test_option_result_distinction_typeck() {
    expect_ok(
        r#"
        fn maybe() -> String? {
            None
        }

        fn load() -> String! {
            Ok("x")
        }

        fn main() {
            print(maybe())
            print(load())
        }
    "#,
    );
}

#[test]
fn test_is_operator_primitives_typeck() {
    expect_ok(
        r#"
        fn main() {
            let x = "abc"
            expect(x is String)
            expect(!(x is Int))
        }
    "#,
    );
}

#[test]
fn test_is_operator_option_typeck() {
    expect_ok(
        r#"
        fn main() {
            let x: Int? = Some(1)
            expect(x is Int?)
        }
    "#,
    );
}

#[test]
fn test_is_operator_result_typeck() {
    expect_ok(
        r#"
        fn main() {
            let x: String! = Ok("hello")
            expect(x is String!)
        }
    "#,
    );
}

#[test]
fn test_match_option() {
    expect_ok(
        r#"
        fn main() {
            let x: String? = Some("hello");
            let y = match x {
                Some(v) => v,
                None => "default",
            };
        }
    "#,
    );
}

#[test]
fn test_match_result() {
    expect_ok(
        r#"
        fn main() {
            let r: String! = Ok("hello");
            let y = match r {
                Ok(v) => v,
                Err(e) => "fallback",
            };
        }
    "#,
    );
}

#[test]
fn test_match_wildcard() {
    expect_ok(
        r#"
        fn main() {
            let x: String? = Some("hello");
            let y = match x {
                _ => "anything",
            };
        }
    "#,
    );
}

#[test]
fn test_match_binding() {
    expect_ok(
        r#"
        fn main() {
            let x: String? = Some("hello");
            let y = match x {
                v => v,
            };
        }
    "#,
    );
}

#[test]
fn test_match_incompatible_arms() {
    expect_err(
        r#"
        fn main() {
            let x: String? = Some("hello");
            let y = match x {
                Some(v) => v,
                None => 123,
            };
        }
    "#,
    );
}

#[test]
fn test_match_scope_isolation() {
    expect_ok(
        r#"
        fn main() {
            let x = "outer";
            let y = match Some("inner") as! String? {
                Some(x) => x,
                None => "none",
            };
            let z = x.len(); // outer x String
        }
    "#,
    );
}

// ── Exhaustiveness checking ────────────────────────────────────────────────

#[test]
fn test_match_exhaustive_option_some_none() {
    expect_ok(
        r#"
        fn main() {
            let x: String? = Some("hello");
            let y = match x {
                Some(v) => v,
                None => "default",
            };
        }
    "#,
    );
}

#[test]
fn test_match_exhaustive_option_wildcard() {
    expect_ok(
        r#"
        fn main() {
            let x: String? = Some("hello");
            let y = match x {
                Some(v) => v,
                _ => "default",
            };
        }
    "#,
    );
}

#[test]
fn test_match_exhaustive_option_binding_catchall() {
    expect_ok(
        r#"
        fn main() {
            let x: String? = Some("hello");
            let y = match x {
                Some(v) => v,
                other => "default",
            };
        }
    "#,
    );
}

#[test]
fn test_match_nonexhaustive_option_missing_none() {
    expect_err(
        r#"
        fn main() {
            let x: String? = Some("hello");
            let y = match x {
                Some(v) => v,
            };
        }
    "#,
    );
}

#[test]
fn test_match_nonexhaustive_option_missing_some() {
    expect_err(
        r#"
        fn main() {
            let x: String? = Some("hello");
            let y = match x {
                None => "default",
            };
        }
    "#,
    );
}

#[test]
fn test_match_duplicate_none_arm() {
    expect_err(
        r#"
        fn main() {
            let x: String? = Some("hello");
            let y = match x {
                None => "a",
                None => "b",
                Some(v) => v,
            };
        }
    "#,
    );
}

#[test]
fn test_match_unreachable_after_wildcard() {
    expect_err(
        r#"
        fn main() {
            let x: String? = Some("hello");
            let y = match x {
                _ => "a",
                None => "b",
            };
        }
    "#,
    );
}

#[test]
fn test_match_unreachable_after_binding_catchall() {
    expect_err(
        r#"
        fn main() {
            let x: String? = Some("hello");
            let y = match x {
                other => "a",
                None => "b",
            };
        }
    "#,
    );
}

#[test]
fn test_match_exhaustive_result_ok_err() {
    expect_ok(
        r#"
        fn main() {
            let r: String! = Ok("hello");
            let y = match r {
                Ok(v) => v,
                Err(e) => "fallback",
            };
        }
    "#,
    );
}

#[test]
fn test_match_nonexhaustive_result_missing_err() {
    expect_err(
        r#"
        fn main() {
            let r: String! = Ok("hello");
            let y = match r {
                Ok(v) => v,
            };
        }
    "#,
    );
}

#[test]
fn test_match_duplicate_ok_arm() {
    expect_err(
        r#"
        fn main() {
            let r: String! = Ok("hello");
            let y = match r {
                Ok(v) => v,
                Ok(w) => w,
                Err(e) => "fallback",
            };
        }
    "#,
    );
}

#[test]
fn test_match_result_catchall_first_unreachable() {
    expect_err(
        r#"
        fn main() {
            let r: String! = Ok("hello");
            let y = match r {
                x => "fallback",
                Err(e) => "never",
            };
        }
    "#,
    );
}

#[test]
fn test_match_unsupported_scrutinee_no_static_check() {
    // Struct scrutinee: no static exhaustiveness, wildcard is fine
    expect_ok(
        r#"
        struct User { name: String }
        fn main() {
            let u = User { name: "a" };
            let y = match u {
                _ => "ok",
            };
        }
    "#,
    );
}

// ── Generalized patterns & literal patterns ────────────────────────────────

#[test]
fn test_match_literal_integer() {
    expect_ok(
        r#"
        fn main() {
            let x = 0;
            let y = match x {
                0 => "zero",
                1 => "one",
                _ => "many",
            };
        }
    "#,
    );
}

#[test]
fn test_match_literal_string() {
    expect_ok(
        r#"
        fn main() {
            let s = "yes";
            let y = match s {
                "yes" => true,
                "no" => false,
                _ => false,
            };
        }
    "#,
    );
}

#[test]
fn test_match_literal_bool() {
    expect_ok(
        r#"
        fn main() {
            let b = true;
            let y = match b {
                true => 1,
                false => 0,
            };
        }
    "#,
    );
}

#[test]
fn test_match_literal_type_mismatch() {
    expect_err(
        r#"
        fn main() {
            let s = "hello";
            let y = match s {
                0 => "zero",
                _ => "other",
            };
        }
    "#,
    );
}

#[test]
fn test_is_uppercase_constructor_pattern_ok() {
    expect_ok(
        r#"
        fn main() {
            let x: String? = Some("hello");
            if x is Some(v) {
                let l = v.len();
            }
        }
    "#,
    );
}

#[test]
fn test_is_naked_lowercase_rejected() {
    expect_err(
        r#"
        fn main() {
            let x = "hello";
            if x is foo { }
        }
    "#,
    );
}

// ── Shape-based pattern & Strict 'is' ──────────────────────────────────────

#[test]
fn test_match_constructor_hardening() {
    // Should FAIL now because foo(a) is not valid for Option
    expect_err(
        r#"
        fn main() {
            let x: Int? = Some(1);
            let y = match x {
                foo(a) => 1,
                _ => 0,
            };
        }
    "#,
    );
}

#[test]
fn test_match_bare_binding_remains_binding() {
    expect_ok(
        r#"
        fn main() {
            let x = 123;
            let y = match x {
                my_binding => my_binding,
            };
            expect(y == 123);
        }
    "#,
    );
}

#[test]
fn test_match_uppercase_fallback_works() {
    expect_ok(
        r#"
        fn main() {
            let x: Int? = Some(1);
            let y = match x {
                None => 0,
                Some(v) => v,
            };
        }
    "#,
    );
}

#[test]
fn test_is_strict_naked_binding_forbidden() {
    // Should fail because 'y' is parsed as a Type, and no such type exists.
    expect_err(
        r#"
        fn main() {
            let x = 1;
            if x is y { }
        }
    "#,
    );
}

#[test]
fn test_is_constructor_hardening() {
    // Should FAIL now because some_ctor(y) is not valid for Option
    expect_err(
        r#"
        fn main() {
            let x: Int? = Some(1);
            if x is some_ctor(y) { }
        }
    "#,
    );
}

#[test]
fn test_is_restricted_struct_pattern_forbidden() {
    // 1) if x is User { name } { } (Must fail)
    expect_err(
        r#"
        struct User { name: String }
        fn main() {
            let x = User { name: "L" };
            if x is User { name } { }
        }
    "#,
    );
}

#[test]
fn test_is_restricted_tuple_pattern_forbidden() {
    // 2) if x is (a, b) { } (Must fail)
    expect_err(
        r#"
        fn main() {
            let x = ("a", 1);
            if x is (a, b) { }
        }
    "#,
    );
}

#[test]
fn test_match_tuple_arity_one_forbidden() {
    // 3) match x { (a) => a } (Must fail)
    expect_err(
        r#"
        fn main() {
            let x = (1, 2);
            match x {
                (a) => a,
                _ => 0,
            };
        }
    "#,
    );
}

#[test]
fn test_match_struct_renaming_forbidden() {
    // 4) match u { User { name: n } => n } (Must fail)
    expect_err(
        r#"
        struct User { name: String }
        fn main() {
            let u = User { name: "L" };
            match u {
                User { name: n } => n,
            };
        }
    "#,
    );
}

#[test]
fn test_match_struct_ok() {
    expect_ok(
        r#"
        struct User { name: String, age: Int }
        fn main() {
            let u = User { name: "L", age: 30 };
            let n = match u {
                User { name, age } => name,
                _ => "default",
            };
        }
    "#,
    );
}

#[test]
fn test_match_tuple_ok() {
    expect_ok(
        r#"
        fn main() {
            let t = ("hello", 42);
            let s = match t {
                (msg, val) => msg,
                _ => "default",
            };
            let v = match t {
                (_, val) => val,
                _ => 0,
            };
        }
    "#,
    );
}

#[test]
fn test_is_none_fallback_works() {
    expect_ok(
        r#"
        fn main() {
            let x: Int? = Some(1);
            if x is None { }
        }
    "#,
    );
}

#[test]
fn test_is_type_uppercase_works() {
    expect_ok(
        r#"
        fn main() {
            let x = 1;
            if x is Int { }
        }
    "#,
    );
}

// ── Bool Exhaustiveness ─────────────────────────────────────────────────────

#[test]
fn test_match_bool_exhaustive() {
    expect_ok(
        r#"
        fn main() {
            let b = true;
            let x = match b {
                true => 1,
                false => 0,
            };
        }
    "#,
    );
}

#[test]
fn test_match_bool_wildcard() {
    expect_ok(
        r#"
        fn main() {
            let b = true;
            let x = match b {
                true => 1,
                _ => 0,
            };
        }
    "#,
    );
}

#[test]
fn test_match_bool_binding_catchall() {
    expect_ok(
        r#"
        fn main() {
            let b = true;
            let x = match b {
                true => 1,
                other => 0,
            };
        }
    "#,
    );
}

#[test]
fn test_match_bool_non_exhaustive_only_true() {
    expect_err(
        r#"
        fn main() {
            let b = true;
            let x = match b {
                true => 1,
            };
        }
    "#,
    );
}

#[test]
fn test_match_bool_non_exhaustive_only_false() {
    expect_err(
        r#"
        fn main() {
            let b = true;
            let x = match b {
                false => 0,
            };
        }
    "#,
    );
}

#[test]
fn test_match_bool_duplicate_true() {
    expect_err(
        r#"
        fn main() {
            let b = true;
            let x = match b {
                true => 1,
                true => 2,
                false => 0,
            };
        }
    "#,
    );
}

#[test]
fn test_match_bool_duplicate_false() {
    expect_err(
        r#"
        fn main() {
            let b = true;
            let x = match b {
                true => 1,
                false => 0,
                false => 2,
            };
        }
    "#,
    );
}

#[test]
fn test_match_bool_unreachable_after_wildcard() {
    expect_err(
        r#"
        fn main() {
            let b = true;
            let x = match b {
                _ => 1,
                true => 2,
            };
        }
    "#,
    );
}

#[test]
fn test_match_bool_unreachable_after_binding() {
    expect_err(
        r#"
        fn main() {
            let b = true;
            let x = match b {
                x => 1,
                false => 0,
            };
        }
    "#,
    );
}

#[test]
fn test_match_bool_false_binding_catchall() {
    expect_ok(
        r#"
        fn main() {
            let b = true;
            let x = match b {
                false => 0,
                x => 1,
            };
        }
    "#,
    );
}
