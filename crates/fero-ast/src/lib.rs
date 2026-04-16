//! Fero AST v0.1
//!
//! This is the first real AST shape for the frontend.
//!
//! Design goals:
//! - easy to read
//! - easy to debug
//! - minimal but not fake
//! - good enough for parser, diagnostics, and later HIR lowering
//!
//! Rules:
//! - every meaningful syntax node carries a span
//! - item nodes model real structure, not placeholders
//! - expression nodes are explicit enough for precedence tests
//! - future features are intentionally omitted

use fero_span::Span;
use fero_lexer::Token;

// ----------------------------------------------------------------------------
// Visibility
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Private,
    Public,
}

// ----------------------------------------------------------------------------
// Root
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct File {
    pub span: Span,
    pub items: Vec<Item>,
    pub top_level_statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub enum Item {
    Import(ImportItem),
    Function(FunctionItem),
    Struct(StructItem),
    Enum(EnumItem),
    Extend(ExtendItem),
}

impl Item {
    pub fn span(&self) -> Span {
        match self {
            Item::Import(item) => item.span,
            Item::Function(item) => item.span,
            Item::Struct(item) => item.span,
            Item::Enum(item) => item.span,
            Item::Extend(item) => item.span,
        }
    }
}

// ----------------------------------------------------------------------------
// Imports
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ImportItem {
    pub span: Span,
    pub path: ImportPath,
}

#[derive(Debug, Clone)]
pub struct ImportPath {
    pub span: Span,
    pub segments: Vec<Token>,
    pub kind: ImportPathKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportPathKind {
    StandardLibrary,
    Relative,
    Package,
}

// ----------------------------------------------------------------------------
// Functions
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FunctionItem {
    pub span: Span,
    pub visibility: Visibility,
    pub name: Token,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<TypeExpr>,
    pub is_fallible: bool,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub span: Span,
    pub name: Token,
    pub ty: TypeExpr,
    pub is_inout: bool,
}

// ----------------------------------------------------------------------------
// Structs
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StructItem {
    pub span: Span,
    pub visibility: Visibility,
    pub name: Token,
    pub fields: Vec<StructField>,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub span: Span,
    pub visibility: Visibility,
    pub name: Token,
    pub ty: TypeExpr,
    pub is_mutable: bool,
    pub default_value: Option<Expression>,
}

// ----------------------------------------------------------------------------
// Enums
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct EnumItem {
    pub span: Span,
    pub visibility: Visibility,
    pub name: Token,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub span: Span,
    pub name: Token,
    pub payload_types: Vec<TypeExpr>,
}

// ----------------------------------------------------------------------------
// Extend
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ExtendItem {
    pub span: Span,
    pub target: TypeExpr,
    pub methods: Vec<FunctionItem>,
}

// ----------------------------------------------------------------------------
// Types
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum TypeExpr {
    Named(NamedTypeExpr),
    Array(ArrayTypeExpr),
    Tuple(TupleTypeExpr),
    Generic(GenericTypeExpr),
}

impl TypeExpr {
    pub fn span(&self) -> Span {
        match self {
            TypeExpr::Named(expr) => expr.span,
            TypeExpr::Array(expr) => expr.span,
            TypeExpr::Tuple(expr) => expr.span,
            TypeExpr::Generic(expr) => expr.span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NamedTypeExpr {
    pub span: Span,
    pub name: Token,
}

#[derive(Debug, Clone)]
pub struct ArrayTypeExpr {
    pub span: Span,
    pub element_type: Box<TypeExpr>,
    pub size: Option<Token>, // integer literal token for fixed-size arrays
}

#[derive(Debug, Clone)]
pub struct TupleTypeExpr {
    pub span: Span,
    pub elements: Vec<TypeExpr>,
}

#[derive(Debug, Clone)]
pub struct GenericTypeExpr {
    pub span: Span,
    pub base: NamedTypeExpr,
    pub arguments: Vec<TypeExpr>,
}

// ----------------------------------------------------------------------------
// Blocks and Statements
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Block {
    pub span: Span,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub enum Statement {
    Let(LetStatement),
    Var(VarStatement),
    Return(ReturnStatement),
    Break(BreakStatement),
    Continue(ContinueStatement),
    For(ForStatement),
    Expression(ExpressionStatement),
}

impl Statement {
    pub fn span(&self) -> Span {
        match self {
            Statement::Let(stmt) => stmt.span,
            Statement::Var(stmt) => stmt.span,
            Statement::Return(stmt) => stmt.span,
            Statement::Break(stmt) => stmt.span,
            Statement::Continue(stmt) => stmt.span,
            Statement::For(stmt) => stmt.span,
            Statement::Expression(stmt) => stmt.span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LetStatement {
    pub span: Span,
    pub name: Token,
    pub initializer: Expression,
}

#[derive(Debug, Clone)]
pub struct VarStatement {
    pub span: Span,
    pub name: Token,
    pub initializer: Expression,
}

#[derive(Debug, Clone)]
pub struct ReturnStatement {
    pub span: Span,
    pub value: Option<Expression>,
}

#[derive(Debug, Clone)]
pub struct BreakStatement {
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ContinueStatement {
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ForStatement {
    pub span: Span,
    pub binding: Token,
    pub iterable: Expression,
    pub filter: Option<Expression>,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct ExpressionStatement {
    pub span: Span,
    pub expression: Expression,
}

// ----------------------------------------------------------------------------
// Expressions
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Expression {
    Identifier(IdentifierExpr),
    Integer(IntegerExpr),
    Float(FloatExpr),
    String(StringExpr),
    Bool(BoolExpr),

    Block(BlockExpr),
    If(IfExpr),
    Match(MatchExpr),
    For(ForExpr),

    Unary(UnaryExpr),
    Binary(BinaryExpr),

    Try(TryExpr),
    Unwrap(UnwrapExpr),

    Call(CallExpr),
    Member(MemberExpr),
    Index(IndexExpr),
    Slice(SliceExpr),

    Tuple(TupleExpr),
    Array(ArrayExpr),
    StructLiteral(StructLiteralExpr),
    Command(CommandExpr),
}

impl Expression {
    pub fn span(&self) -> Span {
        match self {
            Expression::Identifier(expr) => expr.span,
            Expression::Integer(expr) => expr.span,
            Expression::Float(expr) => expr.span,
            Expression::String(expr) => expr.span,
            Expression::Bool(expr) => expr.span,

            Expression::Block(expr) => expr.span,
            Expression::If(expr) => expr.span,
            Expression::Match(expr) => expr.span,
            Expression::For(expr) => expr.span,

            Expression::Unary(expr) => expr.span,
            Expression::Binary(expr) => expr.span,

            Expression::Try(expr) => expr.span,
            Expression::Unwrap(expr) => expr.span,

            Expression::Call(expr) => expr.span,
            Expression::Member(expr) => expr.span,
            Expression::Index(expr) => expr.span,
            Expression::Slice(expr) => expr.span,

            Expression::Tuple(expr) => expr.span,
            Expression::Array(expr) => expr.span,
            Expression::StructLiteral(expr) => expr.span,
            Expression::Command(expr) => expr.span,
        }
    }
}

// ----------------------------------------------------------------------------
// Atomic expressions
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct IdentifierExpr {
    pub span: Span,
    pub name: Token,
}

#[derive(Debug, Clone)]
pub struct IntegerExpr {
    pub span: Span,
    pub token: Token,
}

#[derive(Debug, Clone)]
pub struct FloatExpr {
    pub span: Span,
    pub token: Token,
}

#[derive(Debug, Clone)]
pub struct StringExpr {
    pub span: Span,
    pub token: Token,
}

#[derive(Debug, Clone)]
pub struct BoolExpr {
    pub span: Span,
    pub token: Token,
}

// ----------------------------------------------------------------------------
// Structured expressions
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BlockExpr {
    pub span: Span,
    pub block: Block,
}

#[derive(Debug, Clone)]
pub struct IfExpr {
    pub span: Span,
    pub condition: Box<Expression>,
    pub then_block: Block,
    pub else_branch: Option<Box<Expression>>, // either another If or a Block expression
}

#[derive(Debug, Clone)]
pub struct MatchExpr {
    pub span: Span,
    pub value: Box<Expression>,
    pub arms: Vec<MatchArm>,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub span: Span,
    pub pattern: Pattern,
    pub value: Expression,
}

#[derive(Debug, Clone)]
pub struct ForExpr {
    pub span: Span,
    pub binding: Token,
    pub iterable: Box<Expression>,
    pub filter: Option<Box<Expression>>,
    pub body: Block,
}

// ----------------------------------------------------------------------------
// Unary / Binary
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct UnaryExpr {
    pub span: Span,
    pub operator: Token,
    pub operand: Box<Expression>,
}

#[derive(Debug, Clone)]
pub struct BinaryExpr {
    pub span: Span,
    pub left: Box<Expression>,
    pub operator: Token,
    pub right: Box<Expression>,
}

#[derive(Debug, Clone)]
pub struct TryExpr {
    pub span: Span,
    pub expression: Box<Expression>,
}

#[derive(Debug, Clone)]
pub struct UnwrapExpr {
    pub span: Span,
    pub expression: Box<Expression>,
}

// ----------------------------------------------------------------------------
// Postfix expressions
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum CallArgument {
    Positional(PositionalArgument),
    Named(NamedArgument),
}

#[derive(Debug, Clone)]
pub struct PositionalArgument {
    pub span: Span,
    pub value: Expression,
}

#[derive(Debug, Clone)]
pub struct NamedArgument {
    pub span: Span,
    pub name: Token,
    pub value: Expression,
}

#[derive(Debug, Clone)]
pub struct CallExpr {
    pub span: Span,
    pub callee: Box<Expression>,
    pub arguments: Vec<CallArgument>,
}

#[derive(Debug, Clone)]
pub struct MemberExpr {
    pub span: Span,
    pub object: Box<Expression>,
    pub member: Token,
}

#[derive(Debug, Clone)]
pub struct IndexExpr {
    pub span: Span,
    pub object: Box<Expression>,
    pub index: Box<Expression>,
}

#[derive(Debug, Clone)]
pub struct SliceExpr {
    pub span: Span,
    pub object: Box<Expression>,
    pub start: Box<Expression>,
    pub end: Box<Expression>,
}

// ----------------------------------------------------------------------------
// Tuple / Array / Struct literal
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TupleExpr {
    pub span: Span,
    pub elements: Vec<Expression>,
}

#[derive(Debug, Clone)]
pub struct ArrayExpr {
    pub span: Span,
    pub elements: Vec<Expression>,
}

#[derive(Debug, Clone)]
pub struct StructLiteralExpr {
    pub span: Span,
    pub name: Token,
    pub fields: Vec<StructFieldValue>,
    pub update_from: Option<Token>,
}

#[derive(Debug, Clone)]
pub struct StructFieldValue {
    pub span: Span,
    pub name: Token,
    pub value: Expression,
}

// ----------------------------------------------------------------------------
// Commands
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CommandExpr {
    pub span: Span,
    pub callee: Token, // usually the `cmd` identifier in v0
    pub arguments: Vec<Expression>,
}

// ----------------------------------------------------------------------------
// Patterns
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Pattern {
    Wildcard(WildcardPattern),
    Identifier(IdentifierPattern),
    Literal(LiteralPattern),
    EnumCase(EnumCasePattern),
}

impl Pattern {
    pub fn span(&self) -> Span {
        match self {
            Pattern::Wildcard(pattern) => pattern.span,
            Pattern::Identifier(pattern) => pattern.span,
            Pattern::Literal(pattern) => pattern.span,
            Pattern::EnumCase(pattern) => pattern.span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WildcardPattern {
    pub span: Span,
    pub token: Token,
}

#[derive(Debug, Clone)]
pub struct IdentifierPattern {
    pub span: Span,
    pub name: Token,
}

#[derive(Debug, Clone)]
pub struct LiteralPattern {
    pub span: Span,
    pub token: Token,
}

#[derive(Debug, Clone)]
pub struct EnumCasePattern {
    pub span: Span,
    pub name: Token,
    pub arguments: Vec<Pattern>,
}
