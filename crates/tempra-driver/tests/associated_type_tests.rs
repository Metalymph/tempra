#[cfg(test)]
mod tests {
    use tempra_driver::run_source as run_code;

    #[test]
    fn test_associated_type_minimal() {
        let code = r#"
            interface Indexable {
                type Output
                fn get(self, idx: Int) -> Output
            }

            struct Grid {
                data: Array(Int)
            }

            extend Grid {
                fn get(self, idx: Int) -> Int {
                    return self.data[idx]
                }
            }

            fn requires_indexable(type T: Indexable, value: T) {
            }

            fn main() {
                let g = Grid { data: [42, 43] }
                requires_indexable[Grid](g)
            }
        "#;
        if let Err(e) = run_code(code) {
            panic!("Test failed with error: {}", e);
        }
    }

    #[test]
    fn test_associated_type_mismatch_fails() {
        let code = r#"
            interface Indexable {
                type Output
                fn get(self, idx: Int) -> Output
            }

            struct Wrong {
                data: Array(String)
            }

            extend Wrong {
                fn get(self, idx: Int) -> String {
                    return self.data[idx]
                }
            }

            fn requires_indexable(type T: Indexable, value: T) {
            }

            fn main() {
                let w = Wrong { data: ["hi"] }
                // This succeeds because Indexable doesn't enforce Output to be Int,
                // and we don't have constraints yet.
                requires_indexable[Wrong](w)
            }
        "#;
        assert!(run_code(code).is_ok());
    }

    #[test]
    fn test_associated_type_conflict_fails() {
        let code = r#"
            interface Conflicting {
                type Output
                fn get1(self) -> Output
                fn get2(self) -> Output
            }

            struct S {}
            extend S {
                fn get1(self) -> Int { return 1 }
                fn get2(self) -> String { return "a" }
            }

            fn requires_conflicting(type T: Conflicting, value: T) {}

            fn main() {
                let s = S {}
                // This should fail because Output is inferred as Int from get1
                // and then contradicts with String from get2.
                requires_conflicting[S](s)
            }
        "#;
        let res = run_code(code);
        assert!(res.is_err());
        let err_str = format!("{:?}", res.err().unwrap());
        assert!(err_str.contains("mismatched requirement for method 'get1'") || err_str.contains("mismatched requirement for method 'get2'"));
    }
}
