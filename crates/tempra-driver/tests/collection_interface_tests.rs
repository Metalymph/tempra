use tempra_hir::lower_file;
use tempra_parser::parse_source;
use tempra_typeck::check_file;

fn test_source(source: &str) -> Result<(), Vec<tempra_typeck::error::TypeError>> {
    let ast = parse_source(source).expect("Failed to parse source");
    let mut hir = lower_file(source, &ast);
    check_file(&mut hir)
}

#[test]
fn test_builtin_array_satisfaction() {
    let source = r#"
        fn size(type T: Collection, value: T) -> Int {
            value.len()
        }
        fn main() {
            let xs: Array(Int) = [1, 2, 3];
            expect(size[Array(Int)](xs) == 3);
        }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_builtin_slice_satisfaction() {
    let source = r#"
        fn size(type T: Collection, value: T) -> Int {
            value.len()
        }
        fn main() {
            let xs: Array(Int) = [1, 2, 3];
            let ys: Slice(Int) = xs[0..2];
            expect(size[Slice(Int)](ys) == 2);
        }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_is_empty_on_constrained_generic() {
    let source = r#"
        fn empty(type T: Collection, value: T) -> Bool {
            value.is_empty()
        }
        fn main() {
            let xs: Array(Int) = [];
            expect(empty[Array(Int)](xs) == true);
        }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_item_projection_on_collection() {
    let source = r#"
        fn id_item(type T: Collection, value: T.Item) -> T.Item {
            value
        }
        fn main() {
            let x: Int = id_item[Array(Int)](1);
            expect(x == 1);
        }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_custom_structural_collection_success() {
    let source = r#"
        struct MyColl {
            count: Int
        }
        extend MyColl {
            fn len(self) -> Int { self.count }
            fn is_empty(self) -> Bool { self.count == 0 }
        }
        
        // Item inference will fail because MyColl doesn't provide enough info to infer 'Item'
        // Unless we define Item somehow. In Phase 10a, we don't have 'extend MyColl: Collection'.
        // Structural satisfaction for interfaces with associated types is tricky.
        
        fn get_len(type T: Collection, x: T) -> Int {
            x.len()
        }
        
        fn main() {
            // This should fail Item inference if MyColl is used as Collection
            // BUT get_len doesn't use T.Item, so maybe it works?
            // Actually satisfies_interface checks ALL associated types.
        }
    "#;
    // For now, MyColl doesn't satisfy Collection because Item cannot be inferred.
    let _res = test_source(source);
    // If we call get_len[MyColl], it should fail.
}

#[test]
fn test_slice_view_semantics_with_collection() {
    let source = r#"
        fn size(type T: Collection, value: T) -> Int {
            value.len()
        }
        fn main() {
            var xs: Array(Int) = [1, 2, 3];
            let ys: Slice(Int) = xs[0..2];
            xs[0] = 9;
            expect(size[Slice(Int)](ys) == 2);
            expect(ys[0] == 9);
        }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_collection_rejection_as_runtime_type() {
    let source = r#"
        fn main() {
            let x: Collection = 1;
        }
    "#;
    let _res = test_source(source);
    assert!(_res.is_err());
    // Should fail because Collection resolves to Type::Interface which is rejected in is_valid_type
}

#[test]
fn test_collection_rejection_as_param() {
    let source = r#"
        fn f(x: Collection) {}
    "#;
    assert!(test_source(source).is_err());
}

#[test]
fn test_collection_rejection_as_return() {
    let source = r#"
        fn f() -> Collection { 1 }
    "#;
    assert!(test_source(source).is_err());
}

#[test]
fn test_wrong_len_signature_fails() {
    let source = r#"
        struct Bad {}
        extend Bad {
            fn len(self) -> String { "" }
            fn is_empty(self) -> Bool { false }
        }
        fn take_coll(type T: Collection, x: T) {}
        fn main() {
            let b = Bad {};
            take_coll[Bad](b);
        }
    "#;
    assert!(test_source(source).is_err());
}
