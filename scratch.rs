fn main() {
    let source = r#"
    fn show(type T: Display, value: T) -> String {
        value.display()
    }
    "#;
    tempra_parser::parse_debug(source);
}
