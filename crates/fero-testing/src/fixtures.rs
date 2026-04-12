#[derive(Debug, Clone)]
pub struct Fixture {
    pub name: &'static str,
    pub path: &'static str,
}

pub fn smoke_fixtures() -> Vec<Fixture> {
    vec![Fixture {
        name: "hello",
        path: "examples/hello.fero",
    }]
}
