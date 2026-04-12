use fero_ast::{Expr, Function, Module};
use fero_span::{SourceId, Span};

pub fn fallback_module() -> Module {
    Module {
        functions: vec![Function {
            name: "main".into(),
            body: Expr::Call {
                callee: "io.println".into(),
                args: vec![Expr::Int(0, Span::new(SourceId(0), 0, 0))],
                span: Span::new(SourceId(0), 0, 0),
            },
        }],
    }
}
