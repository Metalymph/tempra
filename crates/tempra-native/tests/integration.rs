use tempra_native::registry::resolve;
use tempra_vm::value::VmValue;

#[test]
fn test_fs_exists() {
    let handler = resolve("std.fs", "exists").expect("exists handler not found");

    // Test for a file that definitely exists (this test file)
    let path = format!("{}/tests/integration.rs", env!("CARGO_MANIFEST_DIR"));
    let args = vec![VmValue::String(path)];
    let res = handler(&args).expect("handler should succeed");

    assert_eq!(res, VmValue::Bool(true));

    // Test for a file that does not exist
    let args_non_exist = vec![VmValue::String("does_not_exist.tmpr".into())];
    let res_non_exist = handler(&args_non_exist).expect("handler should succeed");

    assert_eq!(res_non_exist, VmValue::Bool(false));
}

#[test]
fn test_path_join() {
    let handler = resolve("std.path", "join").expect("join handler not found");

    let args = vec![
        VmValue::String("foo".into()),
        VmValue::String("bar.txt".into()),
    ];
    let res = handler(&args).expect("handler should succeed");

    if let VmValue::String(s) = res {
        // Platform specific separator
        let sep = std::path::MAIN_SEPARATOR.to_string();
        assert_eq!(s, format!("foo{}bar.txt", sep));
    } else {
        panic!("Expected string");
    }
}

#[test]
fn test_path_basename() {
    let handler = resolve("std.path", "basename").expect("basename handler not found");

    let args = vec![VmValue::String("foo/bar.txt".into())];
    let res = handler(&args).expect("handler should succeed");

    if let VmValue::String(s) = res {
        assert_eq!(s, "bar.txt");
    } else {
        panic!("Expected string");
    }
}

#[test]
fn test_strings_helpers() {
    let contains_handler = resolve("std.strings", "contains").expect("handler not found");
    let starts_handler = resolve("std.strings", "starts_with").expect("handler not found");
    let ends_handler = resolve("std.strings", "ends_with").expect("handler not found");

    let s = VmValue::String("Hello, Tempra!".into());

    // Contains
    let args1 = vec![s.clone(), VmValue::String("Tempra".into())];
    assert_eq!(contains_handler(&args1).unwrap(), VmValue::Bool(true));
    let args2 = vec![s.clone(), VmValue::String("foo".into())];
    assert_eq!(contains_handler(&args2).unwrap(), VmValue::Bool(false));

    // Starts with
    let args3 = vec![s.clone(), VmValue::String("Hello".into())];
    assert_eq!(starts_handler(&args3).unwrap(), VmValue::Bool(true));

    // Ends with
    let args4 = vec![s.clone(), VmValue::String("!".into())];
    assert_eq!(ends_handler(&args4).unwrap(), VmValue::Bool(true));
}

#[test]
fn test_io_smoke() {
    let print_handler = resolve("std.io", "print").expect("handler not found");
    let println_handler = resolve("std.io", "println").expect("handler not found");

    // We just smoke test that they don't panic or error out
    let args = vec![VmValue::String("test smoke output".into())];
    let res1 = print_handler(&args).unwrap();
    assert_eq!(res1, VmValue::Unit);

    let res2 = println_handler(&args).unwrap();
    assert_eq!(res2, VmValue::Unit);
}

#[test]
fn test_fs_read_text_ok() {
    let handler = resolve("std.fs", "read_text").expect("read_text handler not found");

    // Use this test file itself — it definitely exists
    let path = format!("{}/tests/integration.rs", env!("CARGO_MANIFEST_DIR"));
    let args = vec![VmValue::String(path)];
    let res = handler(&args).expect("handler should not return Err");

    // Should be Ok(String)
    assert!(
        matches!(&res, VmValue::Enum(e, v, p) if e == "Result" && v == "Ok" && p.len() == 1),
        "Expected Ok(String), got {:?}",
        res
    );
    if let VmValue::Enum(_, _, payloads) = &res {
        assert!(matches!(&payloads[0], VmValue::String(_)));
    }
}

#[test]
fn test_fs_read_text_missing() {
    let handler = resolve("std.fs", "read_text").expect("read_text handler not found");

    let args = vec![VmValue::String("does_not_exist_xyz.txt".into())];
    let res = handler(&args).expect("handler should not panic");

    // Should be Err(String)
    assert!(
        matches!(&res, VmValue::Enum(e, v, p) if e == "Result" && v == "Err" && p.len() == 1),
        "Expected Err(String), got {:?}",
        res
    );
    if let VmValue::Enum(_, _, payloads) = &res {
        assert!(matches!(&payloads[0], VmValue::String(_)));
    }
}

#[test]
fn test_fs_write_text_ok() {
    let handler = resolve("std.fs", "write_text").expect("write_text handler not found");

    let dir = tempfile::tempdir().expect("tempdir failed");
    let out_path = dir.path().join("out.txt");
    let args = vec![
        VmValue::String(out_path.to_string_lossy().to_string()),
        VmValue::String("hello from native".into()),
    ];
    let res = handler(&args).expect("handler should not panic");

    // Should be Ok(Unit)
    assert!(
        matches!(&res, VmValue::Enum(e, v, p) if e == "Result" && v == "Ok" && p.len() == 1),
        "Expected Ok(Unit), got {:?}",
        res
    );
    if let VmValue::Enum(_, _, payloads) = &res {
        assert_eq!(payloads[0], VmValue::Unit);
    }
    assert_eq!(
        std::fs::read_to_string(&out_path).unwrap(),
        "hello from native"
    );
}

#[test]
fn test_fs_write_text_invalid_path() {
    let handler = resolve("std.fs", "write_text").expect("write_text handler not found");

    // Non-existent parent directory
    let args = vec![
        VmValue::String("/nonexistent_dir_xyz/out.txt".into()),
        VmValue::String("data".into()),
    ];
    let res = handler(&args).expect("handler should not panic");

    // Should be Err(String)
    assert!(
        matches!(&res, VmValue::Enum(e, v, p) if e == "Result" && v == "Err" && p.len() == 1),
        "Expected Err(String), got {:?}",
        res
    );
}

#[test]
fn test_strings_trim() {
    let handler = resolve("std.strings", "trim").expect("trim handler not found");

    let args = vec![VmValue::String("  hello world  ".into())];
    let res = handler(&args).unwrap();
    assert_eq!(res, VmValue::String("hello world".into()));

    // Already trimmed
    let args2 = vec![VmValue::String("no_spaces".into())];
    let res2 = handler(&args2).unwrap();
    assert_eq!(res2, VmValue::String("no_spaces".into()));

    // Empty string
    let args3 = vec![VmValue::String("".into())];
    let res3 = handler(&args3).unwrap();
    assert_eq!(res3, VmValue::String("".into()));

    // Only whitespace
    let args4 = vec![VmValue::String("   \t\n  ".into())];
    let res4 = handler(&args4).unwrap();
    assert_eq!(res4, VmValue::String("".into()));
}

#[test]
fn test_strings_replace() {
    let handler = resolve("std.strings", "replace").expect("replace handler not found");

    let args = vec![
        VmValue::String("hello world".into()),
        VmValue::String("world".into()),
        VmValue::String("Tempra".into()),
    ];
    let res = handler(&args).unwrap();
    assert_eq!(res, VmValue::String("hello Tempra".into()));

    // No match — returns unchanged
    let args2 = vec![
        VmValue::String("abc".into()),
        VmValue::String("xyz".into()),
        VmValue::String("123".into()),
    ];
    let res2 = handler(&args2).unwrap();
    assert_eq!(res2, VmValue::String("abc".into()));

    // Multiple occurrences
    let args3 = vec![
        VmValue::String("aaa".into()),
        VmValue::String("a".into()),
        VmValue::String("bb".into()),
    ];
    let res3 = handler(&args3).unwrap();
    assert_eq!(res3, VmValue::String("bbbbbb".into()));
}
