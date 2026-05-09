use tempra_driver::run_source;

fn main() {
    let source = r#"
        struct CounterIter {
            current: Int
            end: Int
        }
        extend CounterIter {
            fn next(inout self) -> Int? {
                if self.current >= self.end {
                    return None;
                }
                let out = self.current;
                self.current = self.current + 1;
                return Some(out);
            }
        }

        struct CounterRange {
            end: Int
        }
        extend CounterRange {
            fn iter(self) -> CounterIter {
                CounterIter { current: 0, end: self.end }
            }
        }

        fn main() {
            let r = CounterRange { end: 3 };
            var sum = 0;
            for x in r {
                sum = sum + x;
            }
            if sum != 3 { crash("sum is not 3") }
        }
    "#;
    match run_source(source) {
        Ok(_) => println!("OK"),
        Err(e) => println!("FAIL: {:?}", e),
    }
}
