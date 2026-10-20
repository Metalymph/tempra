//! Tempra AST v0.1
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

use tempra_lexer::Token;
use tempra_span::Span;

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
    Interface(InterfaceItem),
}

impl Item {
    pub fn span(&self) -> Span {
        match self {
            Item::Import(item) => item.span,
            Item::Function(item) => item.span,
            Item::Struct(item) => item.span,
            Item::Enum(item) => item.span,
            Item::Extend(item) => item.span,
            Item::Interface(item) => item.span,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    Ref,
    Inout,
    Consume,
}

#[derive(Debug, Clone)]
pub enum Capture {
    Named(CaptureNamed),
    RecursiveSelf(Span),
}

#[derive(Debug, Clone)]
pub struct CaptureNamed {
    pub span: Span,
    pub mode: CaptureMode,
    pub name: Token,
}

impl Capture {
    pub fn span(&self) -> Span {
        match self {
            Capture::Named(n) => n.span,
            Capture::RecursiveSelf(s) => *s,
        }
    }
}

// ----------------------------------------------------------------------------
// Functions
// ----------------------------------------------------------------------------

/// Validated public spellings in `@deny`, not analyzer capabilities.
/// Declaration order is the normalization order of the syntax list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DenyCapability {
    /// The `alloc` spelling.
    Alloc,
    /// The `panic` spelling.
    Panic,
    /// The `io` spelling.
    Io,
    /// The `process` spelling.
    Process,
    /// The `spawn` spelling.
    Spawn,
    /// The `nondeterminism` spelling.
    Nondeterminism,
}

impl DenyCapability {
    /// Recognizes a public deny argument. This performs syntax validation only.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "alloc" => Some(Self::Alloc),
            "panic" => Some(Self::Panic),
            "io" => Some(Self::Io),
            "process" => Some(Self::Process),
            "spawn" => Some(Self::Spawn),
            "nondeterminism" => Some(Self::Nondeterminism),
            _ => None,
        }
    }
}

/// Validated specialized operation spelling in `@deny` clause syntax.
///
/// Declaration order mirrors the closed specialized-effect taxonomy and is the
/// canonical operation order. This is syntax data only; it performs no effect
/// analysis and grants no runtime capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SpecializedOperation {
    /// `fs.read`.
    FsRead,
    /// `fs.write`.
    FsWrite,
    /// `fs.remove`.
    FsRemove,
    /// `net.connect`.
    NetConnect,
    /// `net.listen`.
    NetListen,
    /// `io.read`.
    IoRead,
    /// `io.write`.
    IoWrite,
    /// `process.exec`.
    ProcessExec,
    /// `process.exit`.
    ProcessExit,
    /// `spawn.task`.
    SpawnTask,
    /// `spawn.actor`.
    SpawnActor,
}

impl SpecializedOperation {
    /// Every operation in canonical taxonomy order.
    pub const ALL: [Self; 11] = [
        Self::FsRead,
        Self::FsWrite,
        Self::FsRemove,
        Self::NetConnect,
        Self::NetListen,
        Self::IoRead,
        Self::IoWrite,
        Self::ProcessExec,
        Self::ProcessExit,
        Self::SpawnTask,
        Self::SpawnActor,
    ];

    /// Recognizes an exact dotted operation spelling.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "fs.read" => Some(Self::FsRead),
            "fs.write" => Some(Self::FsWrite),
            "fs.remove" => Some(Self::FsRemove),
            "net.connect" => Some(Self::NetConnect),
            "net.listen" => Some(Self::NetListen),
            "io.read" => Some(Self::IoRead),
            "io.write" => Some(Self::IoWrite),
            "process.exec" => Some(Self::ProcessExec),
            "process.exit" => Some(Self::ProcessExit),
            "spawn.task" => Some(Self::SpawnTask),
            "spawn.actor" => Some(Self::SpawnActor),
            _ => None,
        }
    }

    /// Returns the stable canonical dotted spelling.
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::FsRead => "fs.read",
            Self::FsWrite => "fs.write",
            Self::FsRemove => "fs.remove",
            Self::NetConnect => "net.connect",
            Self::NetListen => "net.listen",
            Self::IoRead => "io.read",
            Self::IoWrite => "io.write",
            Self::ProcessExec => "process.exec",
            Self::ProcessExit => "process.exit",
            Self::SpawnTask => "spawn.task",
            Self::SpawnActor => "spawn.actor",
        }
    }

    /// Returns the parameter keys accepted by this operation.
    pub const fn valid_parameter_keys(self) -> &'static [SpecializedParameter] {
        match self {
            Self::FsRead => &[SpecializedParameter::Source],
            Self::FsWrite | Self::FsRemove => &[SpecializedParameter::Destination],
            Self::NetConnect | Self::NetListen => &[
                SpecializedParameter::Host,
                SpecializedParameter::Port,
                SpecializedParameter::Protocol,
            ],
            Self::IoRead | Self::IoWrite => &[SpecializedParameter::Stream],
            Self::ProcessExec => &[SpecializedParameter::Executable],
            Self::ProcessExit => &[],
            Self::SpawnTask | Self::SpawnActor => &[SpecializedParameter::Scope],
        }
    }

    /// Returns whether this operation accepts `parameter`.
    pub fn accepts_parameter(self, parameter: SpecializedParameter) -> bool {
        self.valid_parameter_keys().contains(&parameter)
    }
}

/// Typed parameter key recognized in a specialized `@deny` clause.
///
/// Declaration order mirrors the existing typed-parameter taxonomy and is the
/// canonical parameter order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SpecializedParameter {
    /// `source`.
    Source,
    /// `destination`.
    Destination,
    /// `host`.
    Host,
    /// `port`.
    Port,
    /// `protocol`.
    Protocol,
    /// `executable`.
    Executable,
    /// `stream`.
    Stream,
    /// `scope`.
    Scope,
}

impl SpecializedParameter {
    /// Every parameter key in canonical order.
    pub const ALL: [Self; 8] = [
        Self::Source,
        Self::Destination,
        Self::Host,
        Self::Port,
        Self::Protocol,
        Self::Executable,
        Self::Stream,
        Self::Scope,
    ];

    /// Recognizes an exact parameter spelling.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "source" => Some(Self::Source),
            "destination" => Some(Self::Destination),
            "host" => Some(Self::Host),
            "port" => Some(Self::Port),
            "protocol" => Some(Self::Protocol),
            "executable" => Some(Self::Executable),
            "stream" => Some(Self::Stream),
            "scope" => Some(Self::Scope),
            _ => None,
        }
    }

    /// Returns the stable canonical spelling.
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Destination => "destination",
            Self::Host => "host",
            Self::Port => "port",
            Self::Protocol => "protocol",
            Self::Executable => "executable",
            Self::Stream => "stream",
            Self::Scope => "scope",
        }
    }
}

/// A literal specialized deny value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecializedValue {
    /// Explicit wildcard `*`.
    Any,
    /// Exact quoted text, retained without its surrounding quotes.
    Text(String),
    /// Exact numeric port.
    Port(u16),
}

/// One `name: value` parameter in a specialized deny clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecializedDenyParameter {
    /// Validated parameter key.
    pub parameter: SpecializedParameter,
    /// Literal or wildcard value.
    pub value: SpecializedValue,
    /// Span of `name: value`.
    pub span: Span,
}

/// One specialized deny clause such as `fs.write(destination: "build/out.txt")`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecializedDenyClause {
    /// Validated operation.
    pub operation: SpecializedOperation,
    /// Source-order parameters; duplicate keys are rejected by the parser.
    pub parameters: Vec<SpecializedDenyParameter>,
    /// Span from the operation name through the closing `)`, or through the
    /// operation name for a zero-parameter clause written without parentheses.
    pub span: Span,
}

/// Syntax-only intent from one or more `@deny` attributes on a function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenyContract {
    /// Sorted, deduplicated public capability spellings. Empty when the
    /// contract contains only specialized clauses.
    pub denied: Vec<DenyCapability>,
    /// Specialized clauses in source order. Canonical ordering and duplicate
    /// elimination happen at the driver boundary.
    pub specialized_denied: Vec<SpecializedDenyClause>,
    /// Complete attribute spans (`@` through `)`), in source order.
    pub attribute_spans: Vec<Span>,
}

#[derive(Debug, Clone)]
pub struct FunctionItem {
    pub span: Span,
    /// Optional validated deny syntax; no capability enforcement is performed.
    pub deny_contract: Option<DenyContract>,
    /// Explicit `@pure` source intent (`@` through `pure`), not expanded capabilities.
    pub pure_span: Option<Span>,
    pub visibility: Visibility,
    pub name: Token,
    pub generic_parameters: Vec<GenericParameter>,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<TypeExpr>,
    pub is_fallible: bool,
    pub is_extern: bool,
    pub body: Option<Block>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterKind {
    Runtime,
    Comptime,
    Type,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub span: Span,
    pub name: Token,
    pub ty: TypeExpr,
    pub is_inout: bool,
    pub kind: ParameterKind,
}

#[derive(Debug, Clone)]
pub struct GenericParameter {
    pub span: Span,
    pub name: Token,
    pub constraint: Option<Token>,
}

// ----------------------------------------------------------------------------
// Interfaces
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct InterfaceItem {
    pub span: Span,
    pub visibility: Visibility,
    pub name: Token,
    pub methods: Vec<InterfaceMethodRequirement>,
    pub associated_types: Vec<InterfaceAssociatedType>,
}

#[derive(Debug, Clone)]
pub struct InterfaceAssociatedType {
    pub span: Span,
    pub name: Token,
}

#[derive(Debug, Clone)]
pub struct InterfaceMethodRequirement {
    pub span: Span,
    pub name: Token,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<TypeExpr>,
}

// ----------------------------------------------------------------------------
// Structs
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StructItem {
    pub span: Span,
    pub visibility: Visibility,
    pub name: Token,
    pub generic_parameters: Vec<GenericParameter>,
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
    pub generic_parameters: Vec<GenericParameter>,
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
    Apply(TypeApplyExpr),
    Optional(OptionalTypeExpr),
    Result(ResultTypeExpr),
    Function(FunctionTypeExpr),
    SelfType(Span),
    Projection(TypeProjectionExpr),
}

impl TypeExpr {
    pub fn span(&self) -> Span {
        match self {
            TypeExpr::Named(expr) => expr.span,
            TypeExpr::Array(expr) => expr.span,
            TypeExpr::Tuple(expr) => expr.span,
            TypeExpr::Apply(expr) => expr.span,
            TypeExpr::Optional(expr) => expr.span,
            TypeExpr::Result(expr) => expr.span,
            TypeExpr::Function(expr) => expr.span,
            TypeExpr::SelfType(span) => *span,
            TypeExpr::Projection(expr) => expr.span,
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
pub struct TypeApplyExpr {
    pub span: Span,
    pub callee: Box<TypeExpr>,
    pub arguments: Vec<TypeExpr>,
}

#[derive(Debug, Clone)]
pub struct OptionalTypeExpr {
    pub span: Span,
    pub inner: Box<TypeExpr>,
}

#[derive(Debug, Clone)]
pub struct ResultTypeExpr {
    pub span: Span,
    pub ok: Box<TypeExpr>,
    pub err: Option<Box<TypeExpr>>,
}

#[derive(Debug, Clone)]
pub struct FunctionTypeExpr {
    pub span: Span,
    pub parameters: Vec<TypeExpr>,
    pub return_type: Box<TypeExpr>,
    pub is_fallible: bool,
}

#[derive(Debug, Clone)]
pub struct TypeProjectionExpr {
    pub span: Span,
    pub base: Box<TypeExpr>,
    pub member: Token,
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
    pub ty: Option<TypeExpr>,
    pub initializer: Expression,
}

#[derive(Debug, Clone)]
pub struct VarStatement {
    pub span: Span,
    pub name: Token,
    pub ty: Option<TypeExpr>,
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
    Is(IsExpr),
    IsPattern(IsPatternExpr),
    AsOptional(AsOptionalExpr),
    AsForced(AsForcedExpr),

    Try(TryExpr),
    Unwrap(UnwrapExpr),

    Call(CallExpr),
    Member(MemberExpr),
    Index(IndexExpr),
    Slice(SliceExpr),

    Tuple(TupleExpr),
    Array(ArrayExpr),
    StructLiteral(StructLiteralExpr),
    Closure(ClosureExpr),
    Command(CommandExpr),
    Comptime(ComptimeExpr),
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
            Expression::Is(expr) => expr.span,
            Expression::IsPattern(expr) => expr.span,
            Expression::AsOptional(expr) => expr.span,
            Expression::AsForced(expr) => expr.span,

            Expression::Try(expr) => expr.span,
            Expression::Unwrap(expr) => expr.span,

            Expression::Call(expr) => expr.span,
            Expression::Member(expr) => expr.span,
            Expression::Index(expr) => expr.span,
            Expression::Slice(expr) => expr.span,

            Expression::Tuple(expr) => expr.span,
            Expression::Array(expr) => expr.span,
            Expression::StructLiteral(expr) => expr.span,
            Expression::Closure(expr) => expr.span,
            Expression::Command(expr) => expr.span,
            Expression::Comptime(expr) => expr.span,
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
pub struct ComptimeExpr {
    pub span: Span,
    pub expression: Box<Expression>,
}

#[derive(Debug, Clone)]
pub struct ClosureExpr {
    pub span: Span,
    pub captures: Vec<Capture>,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<TypeExpr>,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct IfExpr {
    pub span: Span,
    pub condition: Box<Expression>,
    pub then_block: Block,
    pub else_branch: Option<Box<Expression>>, // either another If or a Block expression
    pub is_comptime: bool,
}

#[derive(Debug, Clone)]
pub struct MatchExpr {
    pub span: Span,
    pub value: Box<Expression>,
    pub arms: Vec<MatchArm>,
    pub is_comptime: bool,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub span: Span,
    pub pattern: Pattern,
    pub guard: Option<Expression>,
    pub value: Expression,
    pub is_else: bool,
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
pub struct IsExpr {
    pub span: Span,
    pub value: Box<Expression>,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone)]
pub struct IsPatternExpr {
    pub span: Span,
    pub value: Box<Expression>,
    pub pattern: Pattern,
}

#[derive(Debug, Clone)]
pub struct AsOptionalExpr {
    pub span: Span,
    pub expression: Box<Expression>,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone)]
pub struct AsForcedExpr {
    pub span: Span,
    pub expression: Box<Expression>,
    pub ty: TypeExpr,
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
    pub type_args: Option<Vec<TypeExpr>>,
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
    pub ty: TypeExpr,
    pub fields: Vec<StructLiteralField>,
    pub update_from: Option<Box<Expression>>,
}

#[derive(Debug, Clone)]
pub struct StructLiteralField {
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
    Struct(StructPattern),
    Tuple(TuplePattern),
    Or(OrPattern),
}

impl Pattern {
    pub fn span(&self) -> Span {
        match self {
            Pattern::Wildcard(pattern) => pattern.span,
            Pattern::Identifier(pattern) => pattern.span,
            Pattern::Literal(pattern) => pattern.span,
            Pattern::EnumCase(pattern) => pattern.span,
            Pattern::Struct(pattern) => pattern.span,
            Pattern::Tuple(pattern) => pattern.span,
            Pattern::Or(pattern) => pattern.span,
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

#[derive(Debug, Clone)]
pub struct StructPattern {
    pub span: Span,
    pub name: Token,
    pub fields: Vec<(Token, Option<Pattern>)>,
}

#[derive(Debug, Clone)]
pub struct TuplePattern {
    pub span: Span,
    pub items: Vec<Pattern>,
}

#[derive(Debug, Clone)]
pub struct OrPattern {
    pub span: Span,
    pub alternatives: Vec<Pattern>,
}
