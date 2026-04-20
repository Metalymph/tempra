//! High-level Intermediate Representation (HIR) for Fero.
//!
//! The HIR is a simplified and normalized version of the AST,
//! stripped of syntax-only details and tokens, making it easier
//! to perform type checking and lowering to MIR.

use fero_ast::Visibility;
use fero_span::Span;

pub struct HirFile {
    pub items: Vec<HirItem>,
    pub top_level_statements: Vec<HirStatement>,
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
    pub visibility: Visibility,
    pub name: String,
    pub parameters: Vec<HirParameter>,
    pub return_type: Option<HirType>,
    pub is_fallible: bool,
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
    pub visibility: Visibility,
    pub name: String,
    pub fields: Vec<HirStructField>,
}

#[derive(Debug, Clone)]
pub struct HirStructField {
    pub span: Span,
    pub visibility: Visibility,
    pub name: String,
    pub ty: HirType,
    pub is_mutable: bool,
    pub default_value: Option<HirExpression>,
}

#[derive(Debug, Clone)]
pub struct HirEnum {
    pub span: Span,
    pub visibility: Visibility,
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
pub struct HirEnumConstructor {
    pub span: Span,
    pub enum_name: String,
    pub variant_name: String,
    pub payloads: Vec<HirExpression>,
}

#[derive(Debug, Clone)]
pub struct HirEnumConstructorPattern {
    pub span: Span,
    pub enum_name: String,
    pub variant_name: String,
    pub payloads: Vec<HirPattern>,
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
    IsType(HirIsTypeExpr),
    IsPattern(HirIsPatternExpr),
    AsOptional(HirAsOptionalExpr),
    AsForced(HirAsForcedExpr),
    Try(HirTryExpr),
    Unwrap(HirUnwrapExpr),

    Call(HirCall),
    Member(HirMember),
    Index(HirIndex),
    Slice(HirSlice),

    Block(HirBlock),
    If(HirIf),
    Match(HirMatch),
    For(HirForExpr),

    Tuple(Span, Vec<HirExpression>),
    Array(Span, Vec<HirExpression>),
    StructLiteral(HirStructLiteral),
    EnumConstructor(HirEnumConstructor),
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
            HirExpression::IsType(e) => e.span,
            HirExpression::IsPattern(e) => e.span,
            HirExpression::AsOptional(e) => e.span,
            HirExpression::AsForced(e) => e.span,
            HirExpression::Try(e) => e.span,
            HirExpression::Call(e) => e.span,
            HirExpression::Member(e) => e.span,
            HirExpression::Index(e) => e.span,
            HirExpression::Slice(e) => e.span,
            HirExpression::Block(e) => e.span,
            HirExpression::If(e) => e.span,
            HirExpression::Match(e) => e.span,
            HirExpression::For(e) => e.span,
            HirExpression::Tuple(span, _) => *span,
            HirExpression::Array(span, _) => *span,
            HirExpression::StructLiteral(e) => e.span,
            HirExpression::Unwrap(e) => e.span,
            HirExpression::EnumConstructor(e) => e.span,
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

#[derive(Debug, Clone)]
pub struct HirIsTypeExpr {
    pub span: Span,
    pub value: Box<HirExpression>,
    pub ty: HirType,
}


#[derive(Debug, Clone)]
pub struct HirIsPatternExpr {
    pub span: Span,
    pub value: Box<HirExpression>,
    pub pattern: HirPattern,
}

#[derive(Debug, Clone)]
pub struct HirAsOptionalExpr {
    pub span: Span,
    pub expr: Box<HirExpression>,
    pub ty: HirType,
}

#[derive(Debug, Clone)]
pub struct HirAsForcedExpr {
    pub span: Span,
    pub expr: Box<HirExpression>,
    pub ty: HirType,
}

#[derive(Debug, Clone)]
pub struct HirTryExpr {
    pub span: Span,
    pub expr: Box<HirExpression>,
}

#[derive(Debug, Clone)]
pub struct HirUnwrapExpr {
    pub span: Span,
    pub expr: Box<HirExpression>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    NotEq,
    Less,
    Greater,
    And,
    Or,
    Pipe,
    ShiftRight,
    Assign,
}

#[derive(Debug, Clone)]
pub enum HirCallArgument {
    Positional(HirExpression),
    Named(HirNamedArgument),
}

#[derive(Debug, Clone)]
pub struct HirNamedArgument {
    pub name: String,
    pub value: HirExpression,
}

#[derive(Debug, Clone)]
pub struct HirCall {
    pub span: Span,
    pub callee: Box<HirExpression>,
    pub args: Vec<HirCallArgument>,
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
pub struct HirSlice {
    pub span: Span,
    pub object: Box<HirExpression>,
    pub start: Box<HirExpression>,
    pub end: Box<HirExpression>,
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
    pub guard: Option<HirExpression>,
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
    Generic(Span, String, Vec<HirType>),
    Option(Span, Box<HirType>),
    Result(Span, Box<HirType>, Option<Box<HirType>>),
}

impl HirType {
    pub fn span(&self) -> Span {
        match self {
            HirType::Named(span, _) => *span,
            HirType::Array(span, _) => *span,
            HirType::Tuple(span, _) => *span,
            HirType::Generic(span, _, _) => *span,
            HirType::Option(span, _) => *span,
            HirType::Result(span, _, _) => *span,
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
    Literal(Span, Box<HirExpression>),
    EnumCase(Span, String, Vec<HirPattern>),
    Struct(Span, String, Vec<(String, Option<HirPattern>)>),
    Tuple(Span, Vec<HirPattern>),
    EnumConstructor(HirEnumConstructorPattern),
}

impl HirPattern {
    pub fn span(&self) -> Span {
        match self {
            HirPattern::Wildcard(span) => *span,
            HirPattern::Identifier(span, _) => *span,
            HirPattern::Literal(span, _) => *span,
            HirPattern::EnumCase(span, _, _) => *span,
            HirPattern::Struct(span, _, _) => *span,
            HirPattern::Tuple(span, _) => *span,
            HirPattern::EnumConstructor(e) => e.span,
        }
    }
}
