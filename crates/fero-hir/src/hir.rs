use fero_span::Span;

pub struct HirFile {
    pub items: Vec<HirItem>,
}

#[derive(Debug, Clone)]
pub enum HirItem {
    Import(HirImport),
    Function(HirFunction),
    Struct(HirStruct),
    Enum(HirEnum),
    Extend(HirExtend),
}

#[derive(Debug, Clone)]
pub struct HirImport {
    pub span: Span,
    pub path: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct HirFunction {
    pub span: Span,
    pub name: String,
    pub parameters: Vec<HirParameter>,
    pub return_type: Option<HirType>,
    pub body: HirBlock,
}

#[derive(Debug, Clone)]
pub struct HirParameter {
    pub span: Span,
    pub name: String,
    pub ty: HirType,
    pub is_inout: bool,
}

#[derive(Debug, Clone)]
pub struct HirStruct {
    pub span: Span,
    pub name: String,
    pub fields: Vec<HirStructField>,
}

#[derive(Debug, Clone)]
pub struct HirStructField {
    pub span: Span,
    pub name: String,
    pub ty: HirType,
    pub is_mutable: bool,
}

#[derive(Debug, Clone)]
pub struct HirEnum {
    pub span: Span,
    pub name: String,
    pub variants: Vec<HirEnumVariant>,
}

#[derive(Debug, Clone)]
pub struct HirEnumVariant {
    pub span: Span,
    pub name: String,
    pub payload: Vec<HirType>,
}

#[derive(Debug, Clone)]
pub struct HirExtend {
    pub span: Span,
    pub target: HirType,
    pub methods: Vec<HirFunction>,
}

#[derive(Debug, Clone)]
pub enum HirStatement {
    Let(HirLet),
    Var(HirVar),
    Return(Option<HirExpression>),
    Break(Span),
    Continue(Span),
    For(HirFor),
    Expr(HirExpression),
}

#[derive(Debug, Clone)]
pub struct HirLet {
    pub span: Span,
    pub name: String,
    pub value: HirExpression,
}

#[derive(Debug, Clone)]
pub struct HirVar {
    pub span: Span,
    pub name: String,
    pub value: HirExpression,
}

#[derive(Debug, Clone)]
pub struct HirFor {
    pub span: Span,
    pub binding: String,
    pub iterable: HirExpression,
    pub filter: Option<HirExpression>,
    pub body: HirBlock,
}

#[derive(Debug, Clone)]
pub enum HirExpression {
    Name(Span, String),
    Integer(Span, String),
    Float(Span, String),
    String(Span, String),
    Bool(Span, bool),

    Unary(HirUnary),
    Binary(HirBinary),

    Call(HirCall),
    Member(HirMember),
    Index(HirIndex),

    Block(HirBlock),
    If(HirIf),
    Match(HirMatch),
    For(HirForExpr),

    Tuple(Span, Vec<HirExpression>),
    Array(Span, Vec<HirExpression>),
    StructLiteral(HirStructLiteral),
}

impl HirExpression {
    pub fn span(&self) -> Span {
        match self {
            HirExpression::Name(span, _) => *span,
            HirExpression::Integer(span, _) => *span,
            HirExpression::Float(span, _) => *span,
            HirExpression::String(span, _) => *span,
            HirExpression::Bool(span, _) => *span,
            HirExpression::Unary(e) => e.span,
            HirExpression::Binary(e) => e.span,
            HirExpression::Call(e) => e.span,
            HirExpression::Member(e) => e.span,
            HirExpression::Index(e) => e.span,
            HirExpression::Block(e) => e.span,
            HirExpression::If(e) => e.span,
            HirExpression::Match(e) => e.span,
            HirExpression::For(e) => e.span,
            HirExpression::Tuple(span, _) => *span,
            HirExpression::Array(span, _) => *span,
            HirExpression::StructLiteral(e) => e.span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HirUnary {
    pub span: Span,
    pub op: HirUnaryOp,
    pub expr: Box<HirExpression>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirUnaryOp {
    Not,
    Negate,
}

#[derive(Debug, Clone)]
pub struct HirBinary {
    pub span: Span,
    pub left: Box<HirExpression>,
    pub op: HirBinaryOp,
    pub right: Box<HirExpression>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirBinaryOp {
    Add, Sub, Mul, Div,
    Eq, NotEq, Less, Greater,
    And, Or, Pipe, ShiftRight,
    Assign,
}

#[derive(Debug, Clone)]
pub struct HirCall {
    pub span: Span,
    pub callee: Box<HirExpression>,
    pub args: Vec<HirExpression>,
}

#[derive(Debug, Clone)]
pub struct HirMember {
    pub span: Span,
    pub object: Box<HirExpression>,
    pub field: String,
}

#[derive(Debug, Clone)]
pub struct HirIndex {
    pub span: Span,
    pub object: Box<HirExpression>,
    pub index: Box<HirExpression>,
}

#[derive(Debug, Clone)]
pub struct HirIf {
    pub span: Span,
    pub condition: Box<HirExpression>,
    pub then_block: HirBlock,
    pub else_branch: Option<Box<HirExpression>>,
}

#[derive(Debug, Clone)]
pub struct HirMatch {
    pub span: Span,
    pub value: Box<HirExpression>,
    pub arms: Vec<HirMatchArm>,
}

#[derive(Debug, Clone)]
pub struct HirMatchArm {
    pub span: Span,
    pub pattern: HirPattern,
    pub value: HirExpression,
}

#[derive(Debug, Clone)]
pub struct HirForExpr {
    pub span: Span,
    pub binding: String,
    pub iterable: Box<HirExpression>,
    pub filter: Option<Box<HirExpression>>,
    pub body: HirBlock,
}

#[derive(Debug, Clone)]
pub struct HirStructLiteral {
    pub span: Span,
    pub name: String,
    pub fields: Vec<(String, HirExpression)>,
}

#[derive(Debug, Clone)]
pub enum HirType {
    Named(Span, String),
    Array(Span, Box<HirType>),
    Tuple(Span, Vec<HirType>),
}

impl HirType {
    pub fn span(&self) -> Span {
        match self {
            HirType::Named(span, _) => *span,
            HirType::Array(span, _) => *span,
            HirType::Tuple(span, _) => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HirBlock {
    pub span: Span,
    pub statements: Vec<HirStatement>,
}

#[derive(Debug, Clone)]
pub enum HirPattern {
    Wildcard(Span),
    Identifier(Span, String),
    Literal(Span, HirExpression),
    EnumCase(Span, String, Vec<HirPattern>),
}

impl HirPattern {
    pub fn span(&self) -> Span {
        match self {
            HirPattern::Wildcard(span) => *span,
            HirPattern::Identifier(span, _) => *span,
            HirPattern::Literal(span, _) => *span,
            HirPattern::EnumCase(span, _, _) => *span,
        }
    }
}
