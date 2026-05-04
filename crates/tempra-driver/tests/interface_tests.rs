use tempra_driver::pipeline::test_source;

#[test]
fn test_interface_declaration() {
    let source = r#"
    interface Display {
        fn display(self) -> String
    }
    fn main() {}
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_interface_structural_satisfaction_success() {
    let source = r#"
    interface Display {
        fn display(self) -> String
    }

    struct Person { name: String }
    extend Person {
        fn display(self) -> String {
            self.name
        }
    }

    fn show(type T: Display, value: T) -> String {
        value.display()
    }

    fn main() {
        let p = Person { name: "Alice" };
        let s = show[Person](p);
        expect(s == "Alice")
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_interface_structural_satisfaction_failure() {
    let source = r#"
    interface Display {
        fn display(self) -> String
    }

    struct Person { name: String }
    // No display method

    fn show(type T: Display, value: T) -> String {
        value.display()
    }

    fn main() {
        let p = Person { name: "Alice" };
        let _s = show[Person](p);
    }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("does not satisfy Display"));
    assert!(err.contains("missing requirement"));
}

#[test]
fn test_interface_signature_mismatch() {
    let source = r#"
    interface Display {
        fn display(self) -> String
    }

    struct Person { name: String }
    extend Person {
        fn display(self) -> Int { // Mismatched return type
            0
        }
    }

    fn show(type T: Display, value: T) -> String {
        value.display()
    }

    fn main() {
        let p = Person { name: "Alice" };
        let _s = show[Person](p);
    }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("mismatched requirement"));
}

#[test]
fn test_interface_runtime_type_rejection() {
    let source = r#"
    interface Display {
        fn display(self) -> String
    }

    fn main() {
        let x: Display = 10;
    }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("interfaces are static constraints and cannot be used as runtime types"));
}

#[test]
fn test_self_usage_success() {
    let source = r#"
    interface Comparable {
        fn compare(self, other: Self) -> Int
    }
    fn main() {}
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_self_usage_failure_outside_interface() {
    let source = r#"
    fn f(x: Self) {} // Not in interface
    fn main() {}
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("'Self' is only allowed within interface requirements"));
}

#[test]
fn test_multiple_methods_satisfaction() {
    let source = r#"
    interface Duck {
        fn quack(self) -> String
        fn walk(self) -> String
    }

    struct Mallard {}
    extend Mallard {
        fn quack(self) -> String { "Quack" }
        fn walk(self) -> String { "Waddle" }
    }

    fn act(type T: Duck, d: T) {
        d.quack();
        d.walk();
        return;
    }

    fn main() {
        act[Mallard](Mallard {})
    }
    "#;
    assert!(test_source(source).is_ok());
}
