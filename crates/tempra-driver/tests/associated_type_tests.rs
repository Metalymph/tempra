use tempra_driver::{run_source, check_source};

#[test]
fn test_associated_type_multiple() {
    let code = r#"
interface Converter {
    type Input
    type Output
    fn convert(self, value: Input) -> Output
}

struct IntToString {}
extend IntToString {
    fn convert(self, value: Int) -> String {
        fmt(value)
    }
}

fn do_convert(type C: Converter, conv: C, val: C.Input) -> C.Output {
    conv.convert(val)
}

fn main() {
    let c = IntToString {}
    let s: String = do_convert(IntToString, c, 123)
    expect(s == "123")
}
"#;
    let result = run_source(code);
    assert!(result.is_ok(), "Expected success, got {:?}", result.err());
}

#[test]
fn test_associated_type_projection_basic() {
    let code = r#"
interface Indexable {
    type Output
    fn get(self, index: Int) -> Output
}

struct Grid {
    data: [Int]
}

extend Grid {
    fn get(self, index: Int) -> Int {
        self.data[index]
    }
}

fn first(type T: Indexable, value: T) -> T.Output {
    value.get(0)
}

fn main() {
    let g = Grid { data: [10, 20, 30] }
    let x: Int = first(Grid, g)
    // We can't easily capture print output here without more setup, 
    // but the fact that it compiles and runs to completion is already a success for type checking.
}
"#;
    let result = run_source(code);
    assert!(result.is_ok(), "Expected success, got {:?}", result.err());
}

#[test]
fn test_associated_type_projection_error_unconstrained() {
    let code = r#"
fn first(type T, value: T) -> T.Output {
    value.get(0)
}
"#;
    let res = check_source(code);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("associated type projection is only valid on constrained generic parameters"));
}

#[test]
fn test_associated_type_projection_error_missing_member() {
    let code = r#"
interface Indexable {
    type Output
}

fn first(type T: Indexable, value: T) -> T.Wrong {
    value
}
"#;
    let res = check_source(code);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("has no interface constraint declaring associated type Wrong"));
}

#[test]
fn test_associated_type_projection_error_runtime() {
    let code = r#"
struct User { name: String }
fn main() {
    let x: User.name = "hello"
}
"#;
    let res = check_source(code);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("associated type projection is only valid on constrained generic parameters"));
}
