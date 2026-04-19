use fero_driver::pipeline::test_source;

#[test]
fn test_builtin_namespace_access() {
    let source = "
    fn main() {
        // We can't easily test real fs.read in these tests without a mock fs,
        // but we can test that it typechecks and reaches the evaluator.
        // Actually, fs.read is a builtin that might attempt real I/O.
        // Let's use os.cwd() which is usually safe to call.
        let cwd = os.cwd()!;
        expect(cwd.len() > 0);
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "Namespace access failed: {:?}", result.err());
}

#[test]
fn test_invalid_member_on_namespace() {
    let source = "
    fn main() {
        fs.nope();
    }
    ";
    let result = test_source(source);
    assert!(result.is_err());
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("Unknown filesystem operation") || err.contains("Unknown namespace member") || err.contains("not found") || err.contains("Undefined"));
}

#[test]
fn test_namespace_shadowing() {
    let source = "
    fn main() {
        let fs = 123;
        expect(fs == 123);
    }
    ";
    let result = test_source(source);
    assert!(result.is_ok(), "Namespace shadowing failed: {:?}", result.err());
}

#[test]
fn test_namespace_shadowing_error() {
    let source = "
    fn main() {
        let fs = 123;
        fs.read(\"foo.txt\");
    }
    ";
    let result = test_source(source);
    assert!(result.is_err(), "Should fail when shadowed namespace is used incorrectly");
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("no method 'read'") || err.contains("not a struct"));
}
