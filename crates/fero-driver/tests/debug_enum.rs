use fero_driver::pipeline::test_source;
fn main() {
    let source = "
    enum A { One }
    enum B { Two }
    fn main() {
        let x = One;
        match x {
            Two => 0,
            One => 1,
        };
    }
    ";
    let result = test_source(source);
    println!("{:?}", result.err().unwrap());
}
