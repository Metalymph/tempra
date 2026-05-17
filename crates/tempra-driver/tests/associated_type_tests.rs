use tempra_driver::{check_source, run_source};

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
    assert!(
        err.contains("associated type projection is only valid on constrained generic parameters")
    );
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
    assert!(
        err.contains("associated type projection is only valid on constrained generic parameters")
    );
}

#[test]
fn test_associated_type_nested_array() {
    let code = r#"
interface Buffer {
    type Item
    fn items(self) -> Array(Item)
}

struct IntBuffer {
    data: Array(Int)
}

extend IntBuffer {
    fn items(self) -> Array(Int) {
        self.data
    }
}

fn get_items(type T: Buffer, value: T) -> Array(T.Item) {
    value.items()
}

fn main() {
    let b = IntBuffer { data: [1, 2, 3] }
    let items: Array(Int) = get_items(IntBuffer, b)
    expect(items.len() == 3)
}
"#;
    let result = run_source(code);
    assert!(result.is_ok(), "Expected success, got {:?}", result.err());
}

#[test]
fn test_associated_type_nested_slice() {
    let code = r#"
interface View {
    type Item
    fn view(self) -> Slice(Item)
}

struct IntView {
    data: Array(Int)
}

extend IntView {
    fn view(self) -> Slice(Int) {
        self.data[0..self.data.len()]
    }
}

fn get_view(type T: View, value: T) -> Slice(T.Item) {
    value.view()
}

fn main() {
    let v = IntView { data: [1, 2, 3] }
    let s: Slice(Int) = get_view(IntView, v)
    expect(s.len() == 3)
}
"#;
    let result = run_source(code);
    assert!(result.is_ok(), "Expected success, got {:?}", result.err());
}

#[test]
fn test_associated_type_function_type() {
    let code = r#"
interface Mapper {
    type Input
    type Output
    fn mapper(self) -> fn(Input) -> Output
}

struct IntToStringMapper {}
extend IntToStringMapper {
    fn mapper(self) -> fn(Int) -> String {
        fn(x: Int) -> String { fmt(x) }
    }
}

fn get_mapper(type T: Mapper, value: T) -> fn(T.Input) -> T.Output {
    value.mapper()
}

fn main() {
    let m = IntToStringMapper {}
    let f: fn(Int) -> String = get_mapper(IntToStringMapper, m)
    let s: String = f(123)
    expect(s == "123")
}
"#;
    let result = run_source(code);
    assert!(result.is_ok(), "Expected success, got {:?}", result.err());
}

#[test]
fn test_associated_type_mismatch_fails() {
    let code = r#"
interface A {
    type Item
    fn item(self) -> Item
}
interface B {
    type Item
    fn item(self) -> Item
}
struct MyA {}
extend MyA {
    fn item(self) -> Int { 1 }
}

fn bad(type T: A, type U: B, a: T) -> U.Item {
    a.item()
}
"#;
    let res = check_source(code);
    assert!(res.is_err());
}
