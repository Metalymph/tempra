use tempra_driver::check_source;

fn main() {
    let source = r#"
        struct MyIter {
            var count: Int
        }
        extend MyIter {
            fn next(inout self) -> Int? {
                if self.count > 0 {
                    self.count = self.count - 1;
                    return Some(self.count);
                }
                return None;
            }
        }

        fn consume_one(type T: Iterator, it: inout T) -> T.Item? {
            it.next()
        }

        fn main() {
            var it = MyIter { count: 10 };
            let _res = consume_one[MyIter](inout it);
        }
    "#;
    match check_source(source) {
        Ok(_) => println!("OK"),
        Err(e) => println!("ERROR: {:?}", e),
    }
}
