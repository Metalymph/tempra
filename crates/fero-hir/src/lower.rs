//! Logic for lowering Fero AST into HIR.

use fero_ast as ast;
use fero_lexer::{Token, TokenKind};
use crate::hir::*;

/// State for the lowering pass.
pub struct Lowerer<'src> {
    source: &'src str,
}

impl<'src> Lowerer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self { source }
    }

    pub fn lower_file(&self, file: &ast::File) -> HirFile {
        HirFile {
            items: file.items.iter().map(|item| self.lower_item(item)).collect(),
            top_level_statements: file.top_level_statements.iter().map(|stmt| self.lower_statement(stmt)).collect(),
        }
    }

    fn text(&self, span: fero_span::Span) -> String {
        self.source[span.start as usize..span.end as usize].to_string()
    }

    fn lower_item(&self, item: &ast::Item) -> HirItem {
        match item {
            ast::Item::Import(item) => HirItem::Import(self.lower_import(item)),
            ast::Item::Function(item) => HirItem::Function(self.lower_function(item)),
            ast::Item::Struct(item) => HirItem::Struct(self.lower_struct(item)),
            ast::Item::Enum(item) => HirItem::Enum(self.lower_enum(item)),
            ast::Item::Extend(item) => HirItem::Extend(self.lower_extend(item)),
        }
    }

    fn lower_import(&self, item: &ast::ImportItem) -> HirImport {
        HirImport {
            span: item.span,
            path: item.path.segments.iter().map(|t| self.text(t.span)).collect(),
        }
    }

    fn lower_function(&self, item: &ast::FunctionItem) -> HirFunction {
        HirFunction {
            span: item.span,
            visibility: item.visibility,
            name: self.text(item.name.span),
            parameters: item.parameters.iter().map(|p| self.lower_parameter(p)).collect(),
            return_type: item.return_type.as_ref().map(|t| self.lower_type(t)),
            is_fallible: item.is_fallible,
            body: self.lower_block(&item.body),
        }
    }

    fn lower_parameter(&self, p: &ast::Parameter) -> HirParameter {
        HirParameter {
            span: p.span,
            name: self.text(p.name.span),
            ty: self.lower_type(&p.ty),
            is_inout: p.is_inout,
        }
    }

    fn lower_struct(&self, item: &ast::StructItem) -> HirStruct {
        HirStruct {
            span: item.span,
            visibility: item.visibility,
            name: self.text(item.name.span),
            fields: item.fields.iter().map(|f| self.lower_struct_field(f)).collect(),
        }
    }

    fn lower_struct_field(&self, f: &ast::StructField) -> HirStructField {
        HirStructField {
            span: f.span,
            visibility: f.visibility,
            name: self.text(f.name.span),
            ty: self.lower_type(&f.ty),
            is_mutable: f.is_mutable,
            default_value: f.default_value.as_ref().map(|e| self.lower_expression(e)),
        }
    }

    fn lower_enum(&self, item: &ast::EnumItem) -> HirEnum {
        HirEnum {
            span: item.span,
            visibility: item.visibility,
            name: self.text(item.name.span),
            variants: item.variants.iter().map(|v| self.lower_enum_variant(v)).collect(),
        }
    }

    fn lower_enum_variant(&self, v: &ast::EnumVariant) -> HirEnumVariant {
        HirEnumVariant {
            span: v.span,
            name: self.text(v.name.span),
            payload: v.payload_types.iter().map(|t| self.lower_type(t)).collect(),
        }
    }

    fn lower_extend(&self, item: &ast::ExtendItem) -> HirExtend {
        HirExtend {
            span: item.span,
            target: self.lower_type(&item.target),
            methods: item.methods.iter().map(|m| self.lower_function(m)).collect(),
        }
    }

    fn lower_type(&self, ty: &ast::TypeExpr) -> HirType {
        match ty {
            ast::TypeExpr::Named(t) => HirType::Named(t.span, self.text(t.name.span)),
            ast::TypeExpr::Array(t) => HirType::Array(t.span, Box::new(self.lower_type(&t.element_type))),
            ast::TypeExpr::Tuple(t) => HirType::Tuple(t.span, t.elements.iter().map(|e| self.lower_type(e)).collect()),
            ast::TypeExpr::Generic(t) => HirType::Generic(
                t.span,
                self.text(t.base.name.span),
                t.arguments.iter().map(|a| self.lower_type(a)).collect(),
            ),
        }
    }

    fn lower_block(&self, block: &ast::Block) -> HirBlock {
        HirBlock {
            span: block.span,
            statements: block.statements.iter().map(|s| self.lower_statement(s)).collect(),
        }
    }

    fn lower_statement(&self, stmt: &ast::Statement) -> HirStatement {
        match stmt {
            ast::Statement::Let(s) => HirStatement::Let(HirLet {
                span: s.span,
                name: self.text(s.name.span),
                value: self.lower_expression(&s.initializer),
            }),
            ast::Statement::Var(s) => HirStatement::Var(HirVar {
                span: s.span,
                name: self.text(s.name.span),
                value: self.lower_expression(&s.initializer),
            }),
            ast::Statement::Return(s) => HirStatement::Return(s.value.as_ref().map(|e| self.lower_expression(e))),
            ast::Statement::Break(s) => HirStatement::Break(s.span),
            ast::Statement::Continue(s) => HirStatement::Continue(s.span),
            ast::Statement::For(s) => HirStatement::For(HirFor {
                span: s.span,
                binding: self.text(s.binding.span),
                iterable: self.lower_expression(&s.iterable),
                filter: s.filter.as_ref().map(|e| self.lower_expression(e)),
                body: self.lower_block(&s.body),
            }),
            ast::Statement::Expression(s) => HirStatement::Expr(self.lower_expression(&s.expression)),
        }
    }

    fn lower_expression(&self, expr: &ast::Expression) -> HirExpression {
        match expr {
            ast::Expression::Identifier(e) => HirExpression::Name(e.span, self.text(e.name.span)),
            ast::Expression::Integer(e) => HirExpression::Integer(e.span, self.text(e.token.span)),
            ast::Expression::Float(e) => HirExpression::Float(e.span, self.text(e.token.span)),
            ast::Expression::String(e) => HirExpression::String(e.span, self.text(e.token.span)),
            ast::Expression::Bool(e) => HirExpression::Bool(e.span, self.text(e.token.span) == "true"),
            ast::Expression::Unary(e) => HirExpression::Unary(HirUnary {
                span: e.span,
                op: match e.operator.kind {
                    TokenKind::Bang => HirUnaryOp::Not,
                    TokenKind::Minus => HirUnaryOp::Negate,
                    _ => unreachable!(),
                },
                expr: Box::new(self.lower_expression(&e.operand)),
            }),
            ast::Expression::Binary(e) => HirExpression::Binary(HirBinary {
                span: e.span,
                left: Box::new(self.lower_expression(&e.left)),
                op: self.lower_binary_op(e.operator),
                right: Box::new(self.lower_expression(&e.right)),
            }),
            ast::Expression::Try(e) => HirExpression::Try(HirTryExpr {
                span: e.span,
                expr: Box::new(self.lower_expression(&e.expression)),
            }),
            ast::Expression::Unwrap(e) => HirExpression::Unwrap(HirUnwrapExpr {
                span: e.span,
                expr: Box::new(self.lower_expression(&e.expression)),
            }),
            ast::Expression::Call(e) => HirExpression::Call(HirCall {
                span: e.span,
                callee: Box::new(self.lower_expression(&e.callee)),
                args: e.arguments.iter().map(|a| match a {
                    ast::CallArgument::Positional(p) => HirCallArgument::Positional(self.lower_expression(&p.value)),
                    ast::CallArgument::Named(n) => HirCallArgument::Named(HirNamedArgument {
                        name: self.text(n.name.span),
                        value: self.lower_expression(&n.value),
                    }),
                }).collect(),
            }),
            ast::Expression::Member(e) => HirExpression::Member(HirMember {
                span: e.span,
                object: Box::new(self.lower_expression(&e.object)),
                field: self.text(e.member.span),
            }),
            ast::Expression::Index(e) => HirExpression::Index(HirIndex {
                span: e.span,
                object: Box::new(self.lower_expression(&e.object)),
                index: Box::new(self.lower_expression(&e.index)),
            }),
            ast::Expression::Slice(e) => HirExpression::Slice(HirSlice {
                span: e.span,
                object: Box::new(self.lower_expression(&e.object)),
                start: Box::new(self.lower_expression(&e.start)),
                end: Box::new(self.lower_expression(&e.end)),
            }),
            ast::Expression::Block(e) => HirExpression::Block(self.lower_block(&e.block)),
            ast::Expression::If(e) => HirExpression::If(HirIf {
                span: e.span,
                condition: Box::new(self.lower_expression(&e.condition)),
                then_block: self.lower_block(&e.then_block),
                else_branch: e.else_branch.as_ref().map(|e| Box::new(self.lower_expression(e))),
            }),
            ast::Expression::Match(e) => HirExpression::Match(HirMatch {
                span: e.span,
                value: Box::new(self.lower_expression(&e.value)),
                arms: e.arms.iter().map(|a| self.lower_match_arm(a)).collect(),
            }),
            ast::Expression::For(e) => HirExpression::For(HirForExpr {
                span: e.span,
                binding: self.text(e.binding.span),
                iterable: Box::new(self.lower_expression(&e.iterable)),
                filter: e.filter.as_ref().map(|f| Box::new(self.lower_expression(f))),
                body: self.lower_block(&e.body),
            }),
            ast::Expression::Tuple(e) => HirExpression::Tuple(e.span, e.elements.iter().map(|el| self.lower_expression(el)).collect()),
            ast::Expression::Array(e) => HirExpression::Array(e.span, e.elements.iter().map(|el| self.lower_expression(el)).collect()),
            ast::Expression::StructLiteral(e) => HirExpression::StructLiteral(HirStructLiteral {
                span: e.span,
                name: self.text(e.name.span),
                fields: e.fields.iter().map(|f| (self.text(f.name.span), self.lower_expression(&f.value))).collect(),
            }),
            ast::Expression::Command(e) => {
                 // Placeholder for command expressions
                 HirExpression::Name(e.span, "COMMAND_NOT_IMPLEMENTED".to_string())
            }
        }
    }

    fn lower_match_arm(&self, arm: &ast::MatchArm) -> HirMatchArm {
        HirMatchArm {
            span: arm.span,
            pattern: self.lower_pattern(&arm.pattern),
            value: self.lower_expression(&arm.value),
        }
    }

    fn lower_pattern(&self, pat: &ast::Pattern) -> HirPattern {
        match pat {
            ast::Pattern::Wildcard(p) => HirPattern::Wildcard(p.span),
            ast::Pattern::Identifier(p) => HirPattern::Identifier(p.span, self.text(p.name.span)),
            ast::Pattern::Literal(p) => HirPattern::Literal(p.span, self.lower_expression_from_token(p.token)),
            ast::Pattern::EnumCase(p) => HirPattern::EnumCase(
                p.span,
                self.text(p.name.span),
                p.arguments.iter().map(|a| self.lower_pattern(a)).collect()
            ),
        }
    }

    fn lower_expression_from_token(&self, token: Token) -> HirExpression {
        match token.kind {
            TokenKind::IntegerLiteral => HirExpression::Integer(token.span, self.text(token.span)),
            TokenKind::FloatLiteral => HirExpression::Float(token.span, self.text(token.span)),
            TokenKind::StringLiteral => HirExpression::String(token.span, self.text(token.span)),
            TokenKind::True => HirExpression::Bool(token.span, true),
            TokenKind::False => HirExpression::Bool(token.span, false),
            _ => unreachable!(),
        }
    }

    fn lower_binary_op(&self, token: Token) -> HirBinaryOp {
        match token.kind {
            TokenKind::Plus => HirBinaryOp::Add,
            TokenKind::Minus => HirBinaryOp::Sub,
            TokenKind::Star => HirBinaryOp::Mul,
            TokenKind::Slash => HirBinaryOp::Div,
            TokenKind::EqualEqual => HirBinaryOp::Eq,
            TokenKind::BangEqual => HirBinaryOp::NotEq,
            TokenKind::Less => HirBinaryOp::Less,
            TokenKind::Greater => HirBinaryOp::Greater,
            TokenKind::AndAnd => HirBinaryOp::And,
            TokenKind::OrOr => HirBinaryOp::Or,
            TokenKind::Pipe => HirBinaryOp::Pipe,
            TokenKind::ShiftRight => HirBinaryOp::ShiftRight,
            TokenKind::Equal => HirBinaryOp::Assign,
            _ => unreachable!("Unexpected binary token: {:?}", token.kind),
        }
    }
}

/// Lowers an entire AST file into a HIR file.
pub fn lower_file(source: &str, ast: &ast::File) -> HirFile {
    let lowerer = Lowerer::new(source);
    lowerer.lower_file(ast)
}
