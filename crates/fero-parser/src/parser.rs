//! Hand-written recursive-descent parser for Fero v0.1.
//!
//! Converts a stream of tokens into a structured Abstract Syntax Tree (AST).

use fero_ast::*;
use fero_lexer::{tokenize, Token, TokenKind};
use fero_span::Span;

/// An error encountered during parsing.
#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

/// The state of the Fero parser.
pub struct Parser<'src> {
    source: &'src str,
    tokens: Vec<Token>,
    position: usize,
}

impl<'src> Parser<'src> {
    /// Creates a new parser for the given source and tokens.
    pub fn new(source: &'src str, tokens: Vec<Token>) -> Self {
        Self {
            source,
            tokens,
            position: 0,
        }
    }

    pub fn text(&self, span: Span) -> &'src str {
        &self.source[span.start as usize..span.end as usize]
    }

    pub fn parse_file(&mut self) -> Result<File, ParseError> {
        let span_start = self.current().span.start;
        let mut items = Vec::new();
        let mut top_level_statements = Vec::new();

        while !self.at(TokenKind::EndOfFile) {
            if self.at(TokenKind::Import)
                || self.at(TokenKind::Fn)
                || self.at(TokenKind::Struct)
                || self.at(TokenKind::Enum)
                || self.at(TokenKind::Extend)
                || self.at(TokenKind::Pub)
            {
                items.push(self.parse_item()?);
            } else {
                top_level_statements.push(self.parse_statement()?);
            }
        }
        let span_end = self.current().span.end;
        Ok(File {
            span: Span::new(span_start, span_end),
            items,
            top_level_statements,
        })
    }

    fn parse_item(&mut self) -> Result<Item, ParseError> {
        let visibility = if self.eat(TokenKind::Pub) {
            Visibility::Public
        } else {
            Visibility::Private
        };

        if self.at(TokenKind::Import) {
            if visibility == Visibility::Public {
                return Err(ParseError {
                    message: "Imports cannot be public".to_string(),
                    span: self.current().span,
                });
            }
            Ok(Item::Import(self.parse_import()?))
        } else if self.at(TokenKind::Fn) {
            Ok(Item::Function(self.parse_function(visibility)?))
        } else if self.at(TokenKind::Struct) {
            Ok(Item::Struct(self.parse_struct(visibility)?))
        } else if self.at(TokenKind::Enum) {
            Ok(Item::Enum(self.parse_enum(visibility)?))
        } else if self.at(TokenKind::Extend) {
            if visibility == Visibility::Public {
                return Err(ParseError {
                    message: "Extend blocks cannot be public".to_string(),
                    span: self.current().span,
                });
            }
            Ok(Item::Extend(self.parse_extend()?))
        } else {
            let token = self.current();
            Err(ParseError {
                message: format!(
                    "Expected item (import, fn, struct, enum, extend), found {:?}",
                    token.kind
                ),
                span: token.span,
            })
        }
    }

    fn parse_import(&mut self) -> Result<ImportItem, ParseError> {
        let token = self.expect(TokenKind::Import)?;
        let mut segments = Vec::new();
        let mut kind = ImportPathKind::Package;
        let mut is_first = true;

        while !self.at(TokenKind::Semicolon) && !self.at(TokenKind::EndOfFile) {
            if is_first && self.at(TokenKind::Dot) {
                let _dot = self.advance();
                self.expect(TokenKind::Slash)?;
                kind = ImportPathKind::Relative;
                is_first = false;
                continue;
            }

            let seg = self.expect_identifier()?;
            if is_first && self.text(seg.span) == "std" {
                kind = ImportPathKind::StandardLibrary;
            }
            segments.push(seg);
            is_first = false;

            if self.eat(TokenKind::Slash) {
                continue;
            } else {
                break;
            }
        }

        let path_span = if let Some(last) = segments.last() {
            Span::new(segments[0].span.start, last.span.end)
        } else {
            token.span
        };

        let end_token = self.current();
        self.eat(TokenKind::Semicolon);

        let path = ImportPath {
            span: path_span,
            segments,
            kind,
        };

        Ok(ImportItem {
            span: Span::new(token.span.start, end_token.span.end),
            path,
        })
    }

    fn parse_function(&mut self, visibility: Visibility) -> Result<FunctionItem, ParseError> {
        let start_token = self.expect(TokenKind::Fn)?;
        let name = self.expect_identifier()?;

        self.expect(TokenKind::LeftParen)?;
        let mut generic_parameters = Vec::new();
        let mut parameters = Vec::new();
        let mut seen_runtime_param = false;

        while !self.at(TokenKind::RightParen) && !self.at(TokenKind::EndOfFile) {
            let p_start = self.current().span.start;

            if self.eat(TokenKind::Type) {
                if seen_runtime_param {
                    return Err(ParseError {
                        message: "generic parameters must appear before runtime parameters".into(),
                        span: self.tokens[self.position - 1].span,
                    });
                }
                let g_name = self.expect_identifier()?;
                let g_end = g_name.span.end;
                generic_parameters.push(GenericParameter {
                    span: Span::new(p_start, g_end),
                    name: g_name,
                });
            } else {
                seen_runtime_param = true;
                let is_inout = self.eat(TokenKind::Inout);
                let p_name = self.expect_identifier()?;
                let p_ty = if self.eat(TokenKind::Colon) {
                    self.parse_type()?
                } else {
                    TypeExpr::Named(NamedTypeExpr {
                        span: p_name.span,
                        name: p_name,
                    })
                };

                let p_end = p_ty.span().end;
                parameters.push(Parameter {
                    span: Span::new(p_start, p_end),
                    name: p_name,
                    ty: p_ty,
                    is_inout,
                });
            }
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RightParen)?;

        let mut is_fallible = false;
        if self.at(TokenKind::Bang) {
            self.advance();
            is_fallible = true;
        }

        let mut return_type = None;
        if self.eat(TokenKind::Arrow) {
            return_type = Some(self.parse_type()?);
        }

        let body = self.parse_block()?;
        let span = Span::new(start_token.span.start, body.span.end);

        Ok(FunctionItem {
            span,
            visibility,
            name,
            generic_parameters,
            parameters,
            return_type,
            is_fallible,
            body,
        })
    }

    fn looks_like_struct_literal_start(&mut self) -> bool {
        if !self.at(TokenKind::LeftBrace) {
            return false;
        }
        let next = self.tokens.get(self.position + 1).map(|t| t.kind);
        let next2 = self.tokens.get(self.position + 2).map(|t| t.kind);
        if next == Some(TokenKind::RightBrace) {
            return true;
        }
        if next == Some(TokenKind::Identifier) && next2 == Some(TokenKind::Colon) {
            return true;
        }
        false
    }

    fn parse_struct_literal_after_type(&mut self, ty: TypeExpr) -> Result<Expression, ParseError> {
        let start_span = ty.span();
        self.expect(TokenKind::LeftBrace)?;
        let mut fields = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::EndOfFile) {
            let f_name = self.expect_identifier()?;
            self.expect(TokenKind::Colon)?;
            let f_val = self.parse_expression()?;
            fields.push(StructLiteralField {
                span: Span::new(f_name.span.start, f_val.span().end),
                name: f_name,
                value: f_val,
            });
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        let end = self.expect(TokenKind::RightBrace)?;
        Ok(Expression::StructLiteral(StructLiteralExpr {
            span: Span::new(start_span.start, end.span.end),
            ty,
            fields,
            update_from: None,
        }))
    }

    fn try_parse_generic_parameters(&mut self) -> Result<Vec<GenericParameter>, ParseError> {
        let mut generic_parameters = Vec::new();
        if self.at(TokenKind::LeftParen) {
            let next = self.tokens.get(self.position + 1).map(|t| t.kind);
            if next == Some(TokenKind::Type) {
                self.advance(); // eat '('
                while !self.at(TokenKind::RightParen) && !self.at(TokenKind::EndOfFile) {
                    let p_start = self.current().span.start;
                    self.expect(TokenKind::Type)?;
                    let g_name = self.expect_identifier()?;
                    generic_parameters.push(GenericParameter {
                        span: Span::new(p_start, g_name.span.end),
                        name: g_name,
                    });
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RightParen)?;
            }
        }
        Ok(generic_parameters)
    }

    fn parse_struct(&mut self, visibility: Visibility) -> Result<StructItem, ParseError> {
        let token = self.expect(TokenKind::Struct)?;
        let name = self.expect_identifier()?;
        let generic_parameters = self.try_parse_generic_parameters()?;
        self.expect(TokenKind::LeftBrace)?;

        let mut fields = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::EndOfFile) {
            let f_start = self.current().span.start;
            let f_visibility = if self.eat(TokenKind::Pub) {
                Visibility::Public
            } else {
                Visibility::Private
            };
            let is_mutable = self.eat(TokenKind::Var);
            let f_name = self.expect_identifier()?;
            self.expect(TokenKind::Colon)?;
            let f_ty = self.parse_type()?;

            let mut default_value = None;
            let mut end_span = f_ty.span().end;
            if self.eat(TokenKind::Equal) {
                let default_expr = self.parse_expression()?;
                end_span = default_expr.span().end;
                default_value = Some(default_expr);
            }

            self.eat(TokenKind::Comma);

            fields.push(StructField {
                span: Span::new(f_start, end_span),
                visibility: f_visibility,
                name: f_name,
                ty: f_ty,
                is_mutable,
                default_value,
            });
        }
        let end_token = self.expect(TokenKind::RightBrace)?;
        Ok(StructItem {
            span: Span::new(token.span.start, end_token.span.end),
            visibility,
            name,
            generic_parameters,
            fields,
        })
    }

    fn parse_enum(&mut self, visibility: Visibility) -> Result<EnumItem, ParseError> {
        let token = self.expect(TokenKind::Enum)?;
        let name = self.expect_identifier()?;
        let generic_parameters = self.try_parse_generic_parameters()?;
        self.expect(TokenKind::LeftBrace)?;

        let mut variants = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::EndOfFile) {
            let v_name = self.expect_identifier()?;
            let v_start = v_name.span.start;
            let mut v_end = v_name.span.end;

            let mut payload_types = Vec::new();
            if self.eat(TokenKind::LeftParen) {
                while !self.at(TokenKind::RightParen) && !self.at(TokenKind::EndOfFile) {
                    payload_types.push(self.parse_type()?);
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
                let end_token = self.expect(TokenKind::RightParen)?;
                v_end = end_token.span.end;
            }
            self.eat(TokenKind::Comma);

            variants.push(EnumVariant {
                span: Span::new(v_start, v_end),
                name: v_name,
                payload_types,
            });
        }
        let end_token = self.expect(TokenKind::RightBrace)?;
        Ok(EnumItem {
            span: Span::new(token.span.start, end_token.span.end),
            visibility,
            name,
            generic_parameters,
            variants,
        })
    }

    fn parse_extend(&mut self) -> Result<ExtendItem, ParseError> {
        let token = self.expect(TokenKind::Extend)?;
        let target_name = self.expect_identifier()?;
        self.expect(TokenKind::LeftBrace)?;

        let mut methods = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::EndOfFile) {
            let v = if self.eat(TokenKind::Pub) {
                Visibility::Public
            } else {
                Visibility::Private
            };
            methods.push(self.parse_function(v)?);
        }
        let end_token = self.expect(TokenKind::RightBrace)?;

        let target = TypeExpr::Named(NamedTypeExpr {
            span: target_name.span,
            name: target_name,
        });

        Ok(ExtendItem {
            span: Span::new(token.span.start, end_token.span.end),
            target,
            methods,
        })
    }

    fn parse_type(&mut self) -> Result<TypeExpr, ParseError> {
        let mut ty = self.parse_type_atom()?;
        let mut consumed_suffix = false;

        if self.eat(TokenKind::QuestionMark) {
            consumed_suffix = true;
            let q = self.tokens[self.position - 1];
            let span = Span::new(ty.span().start, q.span.end);
            ty = TypeExpr::Optional(OptionalTypeExpr {
                span,
                inner: Box::new(ty),
            });
        } else if self.eat(TokenKind::Bang) {
            consumed_suffix = true;
            let bang = self.tokens[self.position - 1];
            let err = if self.at(TokenKind::Identifier)
                || self.at(TokenKind::LeftBracket)
                || self.at(TokenKind::LeftParen)
            {
                Some(Box::new(self.parse_type_atom()?))
            } else {
                None
            };
            let end = err.as_ref().map(|e| e.span().end).unwrap_or(bang.span.end);
            let span = Span::new(ty.span().start, end);
            ty = TypeExpr::Result(ResultTypeExpr {
                span,
                ok: Box::new(ty),
                err,
            });
        }

        if consumed_suffix && (self.at(TokenKind::QuestionMark) || self.at(TokenKind::Bang)) {
            return Err(ParseError {
                message: "chained type suffixes (? / !) are not supported in this position"
                    .to_string(),
                span: self.current().span,
            });
        }

        Ok(ty)
    }

    fn parse_type_atom(&mut self) -> Result<TypeExpr, ParseError> {
        if self.at(TokenKind::Fn) {
            let start = self.advance();
            self.expect(TokenKind::LeftParen)?;
            let mut parameters = Vec::new();
            while !self.at(TokenKind::RightParen) && !self.at(TokenKind::EndOfFile) {
                parameters.push(self.parse_type()?);
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RightParen)?;
            self.expect(TokenKind::Arrow)?;
            let return_type = Box::new(self.parse_type()?);
            let span = Span::new(start.span.start, return_type.span().end);
            Ok(TypeExpr::Function(FunctionTypeExpr {
                span,
                parameters,
                return_type,
                is_fallible: false,
            }))
        } else if self.at(TokenKind::LeftBracket) {
            let start = self.advance();
            let element_type = Box::new(self.parse_type()?);
            let mut size = None;
            if self.eat(TokenKind::Semicolon) {
                size = Some(self.expect(TokenKind::IntegerLiteral)?);
            }
            let end = self.expect(TokenKind::RightBracket)?;
            Ok(TypeExpr::Array(ArrayTypeExpr {
                span: Span::new(start.span.start, end.span.end),
                element_type,
                size,
            }))
        } else if self.at(TokenKind::LeftParen) {
            let start = self.advance();
            let mut elements = Vec::new();
            let mut trailing_comma = false;
            while !self.at(TokenKind::RightParen) && !self.at(TokenKind::EndOfFile) {
                elements.push(self.parse_type()?);
                if self.eat(TokenKind::Comma) {
                    trailing_comma = true;
                } else {
                    break;
                }
            }
            let end = self.expect(TokenKind::RightParen)?;
            if elements.len() == 1 && !trailing_comma {
                // It's a grouped type, just return the inner type but with the outer span
                let inner = elements.remove(0);
                // We don't have a Group variant in TypeExpr, so we just return the inner one.
                // This is fine as parentheses are just for grouping here.
                Ok(inner)
            } else {
                Ok(TypeExpr::Tuple(TupleTypeExpr {
                    span: Span::new(start.span.start, end.span.end),
                    elements,
                }))
            }
        } else {
            let name = self.expect_identifier()?;
            let mut ty = TypeExpr::Named(NamedTypeExpr {
                span: name.span,
                name: name.clone(),
            });
            if self.at(TokenKind::LeftParen) {
                let _start = self.advance();
                let mut arguments = Vec::new();
                while !self.at(TokenKind::RightParen) && !self.at(TokenKind::EndOfFile) {
                    arguments.push(self.parse_type()?);
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
                let end = self.expect(TokenKind::RightParen)?;
                ty = TypeExpr::Apply(TypeApplyExpr {
                    span: Span::new(name.span.start, end.span.end),
                    callee: Box::new(ty),
                    arguments,
                });
            }
            Ok(ty)
        }
    }

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        let start_token = self.expect(TokenKind::LeftBrace)?;
        let mut statements = Vec::new();

        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::EndOfFile) {
            statements.push(self.parse_statement()?);
        }

        let end_token = self.expect(TokenKind::RightBrace)?;

        Ok(Block {
            span: Span::new(start_token.span.start, end_token.span.end),
            statements,
        })
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        if self.at(TokenKind::Let) {
            let start_token = self.advance();
            let name = self.expect_identifier()?;
            let ty = if self.eat(TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };
            self.expect(TokenKind::Equal)?;
            let initializer = self.parse_expression()?;
            let end_token = self.current();
            self.eat(TokenKind::Semicolon);
            Ok(Statement::Let(LetStatement {
                span: Span::new(start_token.span.start, end_token.span.end),
                name,
                ty,
                initializer,
            }))
        } else if self.at(TokenKind::Var) {
            let start_token = self.advance();
            let name = self.expect_identifier()?;
            let ty = if self.eat(TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };
            self.expect(TokenKind::Equal)?;
            let initializer = self.parse_expression()?;
            let end_token = self.current();
            self.eat(TokenKind::Semicolon);
            Ok(Statement::Var(VarStatement {
                span: Span::new(start_token.span.start, end_token.span.end),
                name,
                ty,
                initializer,
            }))
        } else if self.at(TokenKind::Return) {
            let start_token = self.advance();
            let value = if !self.at(TokenKind::Semicolon) && !self.at(TokenKind::RightBrace) {
                Some(self.parse_expression()?)
            } else {
                None
            };
            let end_token = self.current();
            self.eat(TokenKind::Semicolon);
            Ok(Statement::Return(ReturnStatement {
                span: Span::new(start_token.span.start, end_token.span.end),
                value,
            }))
        } else if self.at(TokenKind::Break) {
            let token = self.advance();
            self.eat(TokenKind::Semicolon);
            Ok(Statement::Break(BreakStatement { span: token.span }))
        } else if self.at(TokenKind::Continue) {
            let token = self.advance();
            self.eat(TokenKind::Semicolon);
            Ok(Statement::Continue(ContinueStatement { span: token.span }))
        } else if self.at(TokenKind::For) {
            let start_token = self.advance();
            let binding = self.expect_identifier()?;
            self.expect(TokenKind::In)?;
            let iterable = self.parse_expression()?;
            let filter = if self.eat(TokenKind::If) {
                Some(self.parse_expression()?)
            } else {
                None
            };
            let body = self.parse_block()?;
            let span = Span::new(start_token.span.start, body.span.end);
            Ok(Statement::For(ForStatement {
                span,
                binding,
                iterable,
                filter,
                body,
            }))
        } else {
            let expression = self.parse_expression()?;
            let end_token = self.current();
            self.eat(TokenKind::Semicolon);
            Ok(Statement::Expression(ExpressionStatement {
                span: Span::new(expression.span().start, end_token.span.end),
                expression,
            }))
        }
    }

    fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        self.parse_binary_expression(0)
    }

    fn parse_binary_expression(&mut self, min_precedence: u8) -> Result<Expression, ParseError> {
        let mut left = self.parse_unary_expression()?;

        while let Some(precedence) = self.get_binary_precedence(self.current().kind) {
            if precedence < min_precedence {
                break;
            }
            let operator = self.advance();
            if operator.kind == TokenKind::Is {
                let is_pattern = {
                    let next = self.current();
                    if next.kind == TokenKind::Underscore {
                        true
                    } else if next.kind == TokenKind::Identifier {
                        let next2 = self.peek(1);
                        // Shape-based primary signal: Identifier + ( is always a constructor pattern.
                        if next2.kind == TokenKind::LeftParen {
                            true
                        } else {
                            // Bare identifier: Priority is Type expression.
                            // Falling back to unit constructor candidate only for Uppercase identifiers
                            // that are not followed by type modifiers.
                            let first_char = self.source[next.span.start as usize..]
                                .chars()
                                .next()
                                .unwrap_or('\0');

                            if first_char.is_uppercase() {
                                // Bare identifier starting with uppercase.
                                // Heuristic: If followed by '?', '!', or '[', it's definitely a complex type (Option, Result, Array).
                                // Otherwise, we treat it as a potential pattern (Enum Constructor or Type Test).
                                // The typechecker will disambiguate based on semantic registration.
                                !matches!(
                                    next2.kind,
                                    TokenKind::QuestionMark
                                        | TokenKind::Bang
                                        | TokenKind::LeftBracket
                                )
                            } else {
                                // Lowercase bare identifier is intentionally forbidden as a binding pattern in 'is'.
                                // It will be parsed as a Type and likely produce a "Type not found" error during typecheck.
                                false
                            }
                        }
                    } else {
                        // next.kind is not Underscore or Identifier (e.g. it is LeftParen for a tuple pattern).
                        // User requirement: no tuple patterns in 'is' yet.
                        false
                    }
                };

                if is_pattern {
                    let pattern = self.parse_pattern_ext(false, false)?;
                    let left_span = left.span();
                    left = Expression::IsPattern(IsPatternExpr {
                        span: Span::new(left_span.start, pattern.span().end),
                        value: Box::new(left),
                        pattern,
                    });
                } else {
                    let ty = self.parse_type()?;
                    let left_span = left.span();
                    left = Expression::Is(IsExpr {
                        span: Span::new(left_span.start, ty.span().end),
                        value: Box::new(left),
                        ty,
                    });
                }
            } else if operator.kind == TokenKind::As {
                if self.eat(TokenKind::QuestionMark) {
                    let ty = self.parse_type()?;
                    let left_span = left.span();
                    left = Expression::AsOptional(AsOptionalExpr {
                        span: Span::new(left_span.start, ty.span().end),
                        expression: Box::new(left),
                        ty,
                    });
                } else if self.eat(TokenKind::Bang) {
                    let ty = self.parse_type()?;
                    let left_span = left.span();
                    left = Expression::AsForced(AsForcedExpr {
                        span: Span::new(left_span.start, ty.span().end),
                        expression: Box::new(left),
                        ty,
                    });
                } else {
                    return Err(ParseError {
                        message: "expected '?' or '!' after 'as'".to_string(),
                        span: operator.span,
                    });
                }
            } else {
                let right = self.parse_binary_expression(precedence + 1)?;
                let left_span = left.span();
                let right_span = right.span();

                left = Expression::Binary(BinaryExpr {
                    span: Span::new(left_span.start, right_span.end),
                    left: Box::new(left),
                    operator,
                    right: Box::new(right),
                });
            }
        }

        Ok(left)
    }

    fn get_binary_precedence(&self, kind: TokenKind) -> Option<u8> {
        match kind {
            TokenKind::Equal => Some(1),
            TokenKind::OrOr => Some(2),
            TokenKind::AndAnd => Some(3),
            TokenKind::EqualEqual | TokenKind::BangEqual | TokenKind::Is | TokenKind::As => Some(4),
            TokenKind::Less | TokenKind::Greater => Some(5),
            TokenKind::Pipe => Some(6),
            TokenKind::ShiftRight => Some(7),
            TokenKind::Plus | TokenKind::Minus => Some(8),
            TokenKind::Star | TokenKind::Slash => Some(9),
            _ => None,
        }
    }

    fn parse_unary_expression(&mut self) -> Result<Expression, ParseError> {
        if self.at(TokenKind::Comptime) {
            let start = self.advance();
            let expr = self.parse_expression()?;
            return Ok(Expression::Comptime(ComptimeExpr {
                span: Span::new(start.span.start, expr.span().end),
                expression: Box::new(expr),
            }));
        }

        if self.at(TokenKind::Bang) || self.at(TokenKind::Minus) {
            let operator = self.advance();
            let operand = Box::new(self.parse_unary_expression()?);
            return Ok(Expression::Unary(UnaryExpr {
                span: Span::new(operator.span.start, operand.span().end),
                operator,
                operand,
            }));
        }
        self.parse_postfix_expression()
    }

    fn parse_call_args(&mut self) -> Result<(Vec<CallArgument>, Token), ParseError> {
        self.expect(TokenKind::LeftParen)?;
        let mut arguments = Vec::new();
        let mut seen_named = false;
        let mut named_keys = std::collections::HashSet::new();

        while !self.at(TokenKind::RightParen) && !self.at(TokenKind::EndOfFile) {
            if self.at(TokenKind::Identifier) && self.peek(1).kind == TokenKind::Equal {
                // Named argument
                seen_named = true;
                let name = self.expect_identifier()?;

                let name_str = self.text(name.span);
                if !named_keys.insert(name_str.to_string()) {
                    return Err(ParseError {
                        message: format!("duplicate named argument '{}'", name_str),
                        span: name.span,
                    });
                }

                self.expect(TokenKind::Equal)?;
                let value = self.parse_expression()?;
                arguments.push(CallArgument::Named(NamedArgument {
                    span: Span::new(name.span.start, value.span().end),
                    name,
                    value,
                }));
            } else {
                // Positional argument
                if seen_named {
                    return Err(ParseError {
                        message: "positional argument cannot follow named arguments".to_string(),
                        span: self.current().span,
                    });
                }
                let value = self.parse_expression()?;
                arguments.push(CallArgument::Positional(PositionalArgument {
                    span: value.span(),
                    value,
                }));
            }
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        let end = self.expect(TokenKind::RightParen)?;
        Ok((arguments, end))
    }

    fn parse_postfix_expression(&mut self) -> Result<Expression, ParseError> {
        let mut expr = self.parse_primary_expression()?;

        loop {
            if self.at(TokenKind::LeftParen) {
                let (arguments, end) = self.parse_call_args()?;
                expr = Expression::Call(CallExpr {
                    span: Span::new(expr.span().start, end.span.end),
                    callee: Box::new(expr),
                    type_args: None,
                    arguments,
                });
            } else if self.at(TokenKind::Dot) {
                self.advance();
                let member = self.expect_identifier()?;
                expr = Expression::Member(MemberExpr {
                    span: Span::new(expr.span().start, member.span.end),
                    object: Box::new(expr),
                    member,
                });
            } else if self.at(TokenKind::LeftBracket) {
                let saved_pos = self.position;
                if let Some(type_args) = self.try_parse_specialization_args() {
                    if self.at(TokenKind::LeftParen) {
                        let (arguments, end) = self.parse_call_args()?;
                        expr = Expression::Call(CallExpr {
                            span: Span::new(expr.span().start, end.span.end),
                            callee: Box::new(expr),
                            type_args: Some(type_args),
                            arguments,
                        });
                        continue;
                    }
                    // Not a call, backtrack to '['
                    self.position = saved_pos;
                }

                self.advance();
                let index_or_start = self.parse_expression()?;
                if self.eat(TokenKind::DotDot) {
                    // Slice
                    let end_expr = self.parse_expression()?;
                    let end_bracket = self.expect(TokenKind::RightBracket)?;
                    expr = Expression::Slice(SliceExpr {
                        span: Span::new(expr.span().start, end_bracket.span.end),
                        object: Box::new(expr),
                        start: Box::new(index_or_start),
                        end: Box::new(end_expr),
                    });
                } else {
                    // Index
                    let end_bracket = self.expect(TokenKind::RightBracket)?;
                    expr = Expression::Index(IndexExpr {
                        span: Span::new(expr.span().start, end_bracket.span.end),
                        object: Box::new(expr),
                        index: Box::new(index_or_start),
                    });
                }
            } else if self.at(TokenKind::QuestionMark) {
                let token = self.advance();
                expr = Expression::Try(TryExpr {
                    span: Span::new(expr.span().start, token.span.end),
                    expression: Box::new(expr),
                });
            } else if self.at(TokenKind::Bang) {
                let token = self.advance();
                expr = Expression::Unwrap(UnwrapExpr {
                    span: Span::new(expr.span().start, token.span.end),
                    expression: Box::new(expr),
                });
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary_expression(&mut self) -> Result<Expression, ParseError> {
        let token = self.current();

        match token.kind {
            TokenKind::Identifier => {
                let pos_before = self.position;
                let ident = self.advance();

                // Check for simple struct literal: Ident {
                let mut is_simple_struct = false;
                if self.at(TokenKind::LeftBrace) {
                    if self.looks_like_struct_literal_start() {
                        is_simple_struct = true;
                    }
                }

                if is_simple_struct {
                    return self.parse_struct_literal_after_type(TypeExpr::Named(NamedTypeExpr {
                        span: ident.span,
                        name: ident,
                    }));
                }

                // Check for generic struct literal: Ident(Types) {
                if self.at(TokenKind::LeftParen) {
                    self.position = pos_before;
                    if let Ok(ty) = self.parse_type_atom() {
                        if self.at(TokenKind::LeftBrace) && self.looks_like_struct_literal_start() {
                            return self.parse_struct_literal_after_type(ty);
                        }
                    }
                    // Reset if it didn't look like a struct literal
                    self.position = pos_before;
                    self.advance(); // re-consume ident
                }

                Ok(Expression::Identifier(IdentifierExpr {
                    span: ident.span,
                    name: ident,
                }))
            }
            TokenKind::IntegerLiteral => {
                self.advance();
                Ok(Expression::Integer(IntegerExpr {
                    span: token.span,
                    token,
                }))
            }
            TokenKind::FloatLiteral => {
                self.advance();
                Ok(Expression::Float(FloatExpr {
                    span: token.span,
                    token,
                }))
            }
            TokenKind::StringLiteral => {
                self.advance();
                Ok(Expression::String(StringExpr {
                    span: token.span,
                    token,
                }))
            }
            TokenKind::True | TokenKind::False => {
                self.advance();
                Ok(Expression::Bool(BoolExpr {
                    span: token.span,
                    token,
                }))
            }
            TokenKind::LeftBracket => {
                let start = self.advance();
                let mut elements = Vec::new();
                while !self.at(TokenKind::RightBracket) && !self.at(TokenKind::EndOfFile) {
                    elements.push(self.parse_expression()?);
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
                let end = self.expect(TokenKind::RightBracket)?;
                Ok(Expression::Array(ArrayExpr {
                    span: Span::new(start.span.start, end.span.end),
                    elements,
                }))
            }
            TokenKind::LeftBrace => {
                let block = self.parse_block()?;
                Ok(Expression::Block(BlockExpr {
                    span: block.span,
                    block,
                }))
            }
            TokenKind::LeftParen => {
                let start = self.advance();
                let mut exprs = Vec::new();
                let mut has_comma = false;

                while !self.at(TokenKind::RightParen) && !self.at(TokenKind::EndOfFile) {
                    exprs.push(self.parse_expression()?);
                    if self.eat(TokenKind::Comma) {
                        has_comma = true;
                    } else {
                        break;
                    }
                }
                let end = self.expect(TokenKind::RightParen)?;
                if has_comma || exprs.len() != 1 {
                    Ok(Expression::Tuple(TupleExpr {
                        span: Span::new(start.span.start, end.span.end),
                        elements: exprs,
                    }))
                } else {
                    Ok(exprs.into_iter().next().unwrap())
                }
            }
            TokenKind::Fn => self.parse_closure_expression(),
            TokenKind::If => {
                let start_token = self.advance();
                let condition = Box::new(self.parse_expression()?);
                let then_block = self.parse_block()?;
                let mut else_branch = None;
                let mut end_span = then_block.span.end;

                if self.eat(TokenKind::Else) {
                    if self.at(TokenKind::If) {
                        let e_expr = self.parse_primary_expression()?;
                        end_span = e_expr.span().end;
                        else_branch = Some(Box::new(e_expr));
                    } else {
                        let e_block = self.parse_block()?;
                        let b_expr = Expression::Block(BlockExpr {
                            span: e_block.span,
                            block: e_block.clone(),
                        });
                        end_span = b_expr.span().end;
                        else_branch = Some(Box::new(b_expr));
                    }
                }
                Ok(Expression::If(IfExpr {
                    span: Span::new(start_token.span.start, end_span),
                    condition,
                    then_block,
                    else_branch,
                    is_comptime: false,
                }))
            }
            TokenKind::Match => {
                let start_token = self.advance();
                let value = Box::new(self.parse_expression()?);
                self.expect(TokenKind::LeftBrace)?;
                let mut arms = Vec::new();
                while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::EndOfFile) {
                    let pat = self.parse_pattern()?;
                    let guard = if self.eat(TokenKind::If) {
                        Some(self.parse_expression()?)
                    } else {
                        None
                    };
                    self.expect(TokenKind::FatArrow)?;
                    let body = self.parse_expression()?;
                    arms.push(MatchArm {
                        span: Span::new(pat.span().start, body.span().end),
                        pattern: pat,
                        guard,
                        value: body,
                    });
                    self.eat(TokenKind::Comma);
                }
                let end_token = self.expect(TokenKind::RightBrace)?;
                Ok(Expression::Match(MatchExpr {
                    span: Span::new(start_token.span.start, end_token.span.end),
                    value,
                    arms,
                    is_comptime: false,
                }))
            }
            TokenKind::For => {
                let start_token = self.advance();
                let binding = self.expect_identifier()?;
                self.expect(TokenKind::In)?;
                let iterable = Box::new(self.parse_expression()?);
                let filter = if self.eat(TokenKind::If) {
                    Some(Box::new(self.parse_expression()?))
                } else {
                    None
                };
                let body = self.parse_block()?;
                Ok(Expression::For(ForExpr {
                    span: Span::new(start_token.span.start, body.span.end),
                    binding,
                    iterable,
                    filter,
                    body,
                }))
            }
            _ => Err(ParseError {
                message: format!("unexpected token in expression: {}", token.kind),
                span: token.span,
            }),
        }
    }

    fn parse_closure_expression(&mut self) -> Result<Expression, ParseError> {
        let start_token = self.expect(TokenKind::Fn)?;
        let mut captures = Vec::new();

        if self.at(TokenKind::LeftBracket) {
            let lb = self.advance();
            if self.at(TokenKind::RightBracket) {
                return Err(ParseError {
                    message: "empty capture list is not allowed; use `fn()` for no captures".into(),
                    span: Span::new(lb.span.start, self.current().span.end),
                });
            }

            while !self.at(TokenKind::RightBracket) && !self.at(TokenKind::EndOfFile) {
                let c_start = self.current().span.start;

                if self.at(TokenKind::Identifier) && self.text(self.current().span) == "self" {
                    let token = self.advance();
                    captures.push(Capture::RecursiveSelf(token.span));
                } else {
                    let mode = if self.eat(TokenKind::Ref) {
                        CaptureMode::Ref
                    } else if self.eat(TokenKind::Inout) {
                        CaptureMode::Inout
                    } else if self.eat(TokenKind::Consume) {
                        CaptureMode::Consume
                    } else {
                        return Err(ParseError {
                            message: "invalid capture mode; expected 'ref', 'inout', or 'consume'"
                                .into(),
                            span: self.current().span,
                        });
                    };

                    let name = self.expect_identifier()?;
                    captures.push(Capture::Named(CaptureNamed {
                        span: Span::new(c_start, name.span.end),
                        mode,
                        name,
                    }));
                }

                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RightBracket)?;
        }

        self.expect(TokenKind::LeftParen)?;
        let mut parameters = Vec::new();
        while !self.at(TokenKind::RightParen) && !self.at(TokenKind::EndOfFile) {
            let p_start = self.current().span.start;
            let is_inout = self.eat(TokenKind::Inout);
            let p_name = self.expect_identifier()?;
            self.expect(TokenKind::Colon)?;
            let p_ty = self.parse_type()?;
            let p_end = p_ty.span().end;

            parameters.push(Parameter {
                span: Span::new(p_start, p_end),
                name: p_name,
                ty: p_ty,
                is_inout,
            });

            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RightParen)?;

        let return_type = if self.eat(TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let body = self.parse_block()?;

        Ok(Expression::Closure(ClosureExpr {
            span: Span::new(start_token.span.start, body.span.end),
            captures,
            parameters,
            return_type,
            body,
        }))
    }

    fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        // By default, we allow everything (used in match arms)
        self.parse_pattern_ext(true, true)
    }

    fn parse_pattern_ext(
        &mut self,
        allow_struct: bool,
        allow_tuple: bool,
    ) -> Result<Pattern, ParseError> {
        let mut node = self.parse_pattern_atom(allow_struct, allow_tuple)?;

        if self.at(TokenKind::Pipe) {
            let mut alternatives = vec![node];
            while self.eat(TokenKind::Pipe) {
                let alt = self.parse_pattern_atom(allow_struct, allow_tuple)?;
                // Flattening
                if let Pattern::Or(or_pat) = alt {
                    alternatives.extend(or_pat.alternatives);
                } else {
                    alternatives.push(alt);
                }
            }
            let first_span = alternatives[0].span();
            let last_span = alternatives.last().unwrap().span();
            node = Pattern::Or(OrPattern {
                span: Span::new(first_span.start, last_span.end),
                alternatives,
            });
        }

        Ok(node)
    }

    fn parse_pattern_atom(
        &mut self,
        allow_struct: bool,
        allow_tuple: bool,
    ) -> Result<Pattern, ParseError> {
        let token = self.current();
        if self.eat(TokenKind::Underscore) {
            Ok(Pattern::Wildcard(WildcardPattern {
                span: token.span,
                token,
            }))
        } else if allow_tuple && self.at(TokenKind::LeftParen) {
            let start = self.advance();
            let mut items = Vec::new();
            while !self.at(TokenKind::RightParen) && !self.at(TokenKind::EndOfFile) {
                // Nested tuple patterns are now supported
                items.push(self.parse_pattern_ext(true, true)?);
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            let end = self.expect(TokenKind::RightParen)?;
            if items.is_empty() {
                return Err(ParseError {
                    message: "empty tuple patterns are not supported".to_string(),
                    span: Span::new(start.span.start, end.span.end),
                });
            }
            if items.len() < 2 {
                return Err(ParseError {
                    message: "single-element tuple patterns are not allowed; use `a` instead"
                        .to_string(),
                    span: Span::new(start.span.start, end.span.end),
                });
            }
            Ok(Pattern::Tuple(TuplePattern {
                span: Span::new(start.span.start, end.span.end),
                items,
            }))
        } else if matches!(
            token.kind,
            TokenKind::IntegerLiteral
                | TokenKind::StringLiteral
                | TokenKind::True
                | TokenKind::False
        ) {
            self.advance();
            Ok(Pattern::Literal(LiteralPattern {
                span: token.span,
                token,
            }))
        } else {
            let name = self.expect_identifier()?;
            if self.eat(TokenKind::LeftParen) {
                let mut arguments = Vec::new();
                while !self.at(TokenKind::RightParen) && !self.at(TokenKind::EndOfFile) {
                    // Nested payload patterns are supported
                    arguments.push(self.parse_pattern_ext(true, true)?);
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
                let end = self.expect(TokenKind::RightParen)?;
                Ok(Pattern::EnumCase(EnumCasePattern {
                    span: Span::new(name.span.start, end.span.end),
                    name,
                    arguments,
                }))
            } else if allow_struct && self.eat(TokenKind::LeftBrace) {
                let mut fields = Vec::new();
                while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::EndOfFile) {
                    let field = self.expect_identifier()?;
                    let pat = if self.eat(TokenKind::Colon) {
                        Some(self.parse_pattern_ext(true, true)?)
                    } else {
                        None
                    };
                    fields.push((field, pat));
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
                let end = self.expect(TokenKind::RightBrace)?;
                Ok(Pattern::Struct(StructPattern {
                    span: Span::new(name.span.start, end.span.end),
                    name,
                    fields,
                }))
            } else {
                // Bare identifier rule:
                // - Uppercase identifier: parsed as a nullary constructor pattern (Unit case fallback).
                // - Lowercase identifier: parsed as an identifier binding pattern.
                // NOTE: This fallback heuristic is temporary for v1 until full symbol resolution is added.
                let first_char = self.source[name.span.start as usize..]
                    .chars()
                    .next()
                    .unwrap_or('\0');
                if first_char.is_uppercase() {
                    Ok(Pattern::EnumCase(EnumCasePattern {
                        span: name.span,
                        name,
                        arguments: vec![],
                    }))
                } else {
                    Ok(Pattern::Identifier(IdentifierPattern {
                        span: name.span,
                        name,
                    }))
                }
            }
        }
    }

    // Helpers
    fn current(&self) -> Token {
        self.peek(0)
    }

    fn peek(&self, offset: usize) -> Token {
        self.tokens
            .get(self.position + offset)
            .copied()
            .unwrap_or_else(|| Token {
                kind: TokenKind::EndOfFile,
                span: Span {
                    start: self.source.len() as u32,
                    end: self.source.len() as u32,
                },
            })
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.current().kind == kind
    }

    fn advance(&mut self) -> Token {
        let token = self.current();
        if token.kind != TokenKind::EndOfFile {
            self.position += 1;
        }
        token
    }

    fn expect(&mut self, kind: TokenKind) -> Result<Token, ParseError> {
        if self.at(kind) {
            Ok(self.advance())
        } else {
            let current = self.current();
            Err(ParseError {
                message: format!("expected {}, but found {}", kind, current.kind),
                span: current.span,
            })
        }
    }

    fn eat(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_identifier(&mut self) -> Result<Token, ParseError> {
        self.expect(TokenKind::Identifier)
    }

    fn try_parse_specialization_args(&mut self) -> Option<Vec<TypeExpr>> {
        let saved_pos = self.position;
        self.advance(); // consume '['

        let mut args = Vec::new();
        let mut is_specialization = true;

        if self.at(TokenKind::RightBracket) {
            // empty foo[]
            self.advance();
            return Some(args);
        }

        while !self.at(TokenKind::RightBracket) && !self.at(TokenKind::EndOfFile) {
            let ty_res = self.parse_type();
            match ty_res {
                Ok(ty) => {
                    // Disambiguate: if it's a lowercase named type, it's likely an expression (variable)
                    if let TypeExpr::Named(NamedTypeExpr { name, .. }) = &ty {
                        let text = self.text(name.span);
                        if text
                            .chars()
                            .next()
                            .map(|c| c.is_lowercase())
                            .unwrap_or(false)
                        {
                            is_specialization = false;
                            break;
                        }
                    }
                    args.push(ty);
                }
                Err(_) => {
                    is_specialization = false;
                    break;
                }
            }
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }

        if is_specialization && self.eat(TokenKind::RightBracket) {
            Some(args)
        } else {
            self.position = saved_pos;
            None
        }
    }
}

pub fn parse_source(source: &str) -> Result<File, ParseError> {
    let tokens = tokenize(source);
    let mut parser = Parser::new(source, tokens);
    parser.parse_file()
}

pub fn parse_debug(source: &str) {
    match parse_source(source) {
        Ok(ast) => println!("{:#?}", ast),
        Err(e) => println!("Parse error: {:?}", e),
    }
}
