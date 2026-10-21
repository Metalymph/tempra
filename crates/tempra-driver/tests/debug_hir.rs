#[test]
fn debug_hir_extern() {
    let source = "pub extern fn join(a: String, b: String) -> String";
    let ast = tempra_parser::parse_source(source).unwrap();
    let hir = tempra_hir::lower_file(source, &ast);
    for item in &hir.items {
        if let tempra_hir::HirItem::Function(f) = item {
            println!("name: {}, is_extern: {}", f.name, f.is_extern);
            assert!(f.is_extern, "Function should be extern");
        }
    }
}
