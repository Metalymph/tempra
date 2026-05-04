use tempra_driver::pipeline::test_source;

fn main() {
    let source = r#"
    enum MaybeInt { None, Some(Int) }
    fn main() {
        let x = Some(3);
        if x is Some(v) {
            expect(v == 3, "v should be bound");
        }
        let z = v; // Should fail
    }
    "#;
    let result = test_source(source);
    match result {
        Ok(_) => println!("SUCCESS"),
        Err(e) => println!("ERROR: {:?}", e),
    }
}
