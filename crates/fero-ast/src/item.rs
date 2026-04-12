use crate::Expr;

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub body: Expr,
}

#[derive(Debug, Clone, Default)]
pub struct Module {
    pub functions: Vec<Function>,
}
