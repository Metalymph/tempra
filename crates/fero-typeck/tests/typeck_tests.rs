use fero_parser::parse_source;
use fero_hir::lower_file;
use fero_typeck::check_file;

fn expect_ok(source: &str) {
    let ast = parse_source(source).unwrap();
    let hir = lower_file(source, &ast);
    if let Err(errors) = check_file(&hir) {
        panic!("Type checking failed for source:\n{}\nErrors: {:?}", source, errors);
    }
}

fn expect_err(source: &str) {
    let ast = parse_source(source).unwrap();
    let hir = lower_file(source, &ast);
    if check_file(&hir).is_ok() {
        panic!("Expected type error, but succeeded for source:\n{}", source);
    }
}

#[test]
fn test_literal_typing() {
    expect_ok(r#"
        fn main() {
            let x = 1;
            let y = 1.5;
            let z = "hello";
            let ok = true;
        }
    "#);
}

#[test]
fn test_arithmetic_success() {
    expect_ok(r#"
        fn main() {
            let x = 1 + 2;
        }
    "#);
}

#[test]
fn test_arithmetic_failure() {
    expect_err(r#"
        fn main() {
            let x = "a" + 1;
        }
    "#);
}

#[test]
fn test_if_success() {
    expect_ok(r#"
        fn main() {
            let x = if true { 1 } else { 2 };
        }
    "#);
}

#[test]
fn test_if_failure() {
    expect_err(r#"
        fn main() {
            let x = if true { 1 } else { "a" };
        }
    "#);
}

#[test]
fn test_function_call_success() {
    expect_ok(r#"
        fn add(a: Int, b: Int) -> Int {
            a + b
        }

        fn main() {
            let x = add(1, 2);
        }
    "#);
}

#[test]
fn test_function_call_arity_failure() {
    expect_err(r#"
        fn add(a: Int, b: Int) -> Int {
            a + b
        }

        fn main() {
            let x = add(1);
        }
    "#);
}

#[test]
fn test_struct_literal_success() {
    expect_ok(r#"
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
    "#);
}

#[test]
fn test_struct_literal_failure() {
    expect_err(r#"
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
    "#);
}

#[test]
fn test_array_success() {
    expect_ok(r#"
        fn main() {
            let xs = [1, 2, 3];
        }
    "#);
}

#[test]
fn test_array_failure() {
    expect_err(r#"
        fn main() {
            let xs = [1, "a", 3];
        }
    "#);
}

#[test]
fn test_for_statement_success() {
    expect_ok(r#"
        fn main() {
            let xs = [1, 2, 3];
            for x in xs {
                x;
            }
        }
    "#);
}

#[test]
fn test_for_expression_success() {
    expect_ok(r#"
        fn main() {
            let xs = [1, 2, 3];
            let ys = for x in xs {
                x + 1
            };
        }
    "#);
}

#[test]
fn test_print_success() {
    expect_ok(r#"
        fn main() {
            print("hello")
            print("a", 1, true)
            print()
        }
    "#);
}

#[test]
fn test_fmt_success() {
    expect_ok(r#"
        fn main() {
            let s = fmt("a", 1, true)
            let s2 = fmt()
        }
    "#);
}

#[test]
fn test_read_line_success() {
    expect_ok(r#"
        fn main() {
            let name = read_line()
            print(name)
        }
    "#);
}

#[test]
fn test_format_return_type() {
    expect_ok(r#"
        fn main() {
            let s = fmt("value=", 5)
            print(s)
        }
    "#);
}

#[test]
fn test_read_line_wrong_arity() {
    expect_err(r#"
        fn main() {
            read_line(1)
        }
    "#);
}

#[test]
fn test_cmd_success() {
    expect_ok(r#"
        fn main() {
            let c = cmd("echo", ["hello"]);
        }
    "#);
}

#[test]
fn test_cmd_wrong_arity() {
    expect_err(r#"
        fn main() {
            let c = cmd("echo");
        }
    "#);
}

#[test]
fn test_cmd_wrong_arg_type() {
    expect_err(r#"
        fn main() {
            let c = cmd("echo", "hello");
        }
    "#);
}

#[test]
fn test_run_method_success() {
    expect_ok(r#"
        fn main() {
            let r = cmd("echo", ["hello"]).run();
        }
    "#);
}

#[test]
fn test_command_pipe_success() {
    expect_ok(r#"
        fn main() {
            let r = cmd("echo", ["hello"]) | cmd("grep", ["h"]);
        }
    "#);
}

#[test]
fn test_read_success() {
    expect_ok(r#"
        fn main() {
            let text = fs.read("notes.txt")!;
            print(text);
        }
    "#);
}

#[test]
fn test_read_wrong_type() {
    expect_err(r#"
        fn main() {
            let text = fs.read(123);
        }
    "#);
}

#[test]
fn test_write_success() {
    expect_ok(r#"
        fn main() {
            fs.write("out.txt", "hello")!;
        }
    "#);
}

#[test]
fn test_write_wrong_type() {
    expect_err(r#"
        fn main() {
            fs.write("out.txt", 123)!;
        }
    "#);
}

#[test]
fn test_append_success() {
    expect_ok(r#"
        fn main() {
            fs.append("log.txt", "done")!;
        }
    "#);
}

#[test]
fn test_append_wrong_type() {
    expect_err(r#"
        fn main() {
            fs.append("log.txt", 123);
        }
    "#);
}

#[test]
fn test_exit_success() {
    expect_ok(r#"
        fn main() {
            exit(1);
        }
    "#);
}

#[test]
fn test_exit_wrong_type() {
    expect_err(r#"
        fn main() {
            exit("1");
        }
    "#);
}

#[test]
fn test_expect_success() {
    expect_ok(r#"
        fn main() {
            expect(true);
            expect(false, "message");
        }
    "#);
}

#[test]
fn test_expect_wrong_type() {
    expect_err(r#"
        fn main() {
            expect(1);
        }
    "#);
    expect_err(r#"
        fn main() {
            expect(false, 1);
        }
    "#);
}

#[test]
fn test_dbg_preserves_type() {
    expect_ok(r#"
        fn main() {
            let x = dbg(1);
            let y = dbg("hello");
            let z = dbg(true);
        }
    "#);
}
