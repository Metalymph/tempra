use std::collections::HashMap;
use fero_span::Span;
use fero_hir::*;
use crate::types::{Type, resolve_named_type};
use crate::error::TypeError;
use crate::env::ScopeStack;

/// Metadata about a function signature used during type checking.
#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub params: Vec<(String, Type)>,
    pub return_type: Type,
}

/// Metadata about a struct definition.
#[derive(Debug, Clone)]
pub struct StructInfo {
    pub fields: HashMap<String, Type>,
}

/// Metadata about an enum definition.
#[derive(Debug, Clone)]
pub struct EnumInfo {
    pub variants: HashMap<String, Vec<Type>>,
}

/// Global type-checking context that is built in a registration pass and
/// then queried during the checking pass.
pub struct TypeContext {
    pub functions: HashMap<String, FunctionSignature>,
    pub structs: HashMap<String, StructInfo>,
    pub enums: HashMap<String, EnumInfo>,
    pub scopes: ScopeStack,
    pub errors: Vec<TypeError>,
    pub current_return_type: Option<Type>,
}

impl TypeContext {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            scopes: ScopeStack::new(),
            errors: Vec::new(),
            current_return_type: None,
        }
    }

    pub fn error(&mut self, message: impl Into<String>, span: Span) {
        self.errors.push(TypeError::new(message, span));
    }

    /// Resolve an HIR type node into a semantic Type.
    pub fn resolve_hir_type(&self, hir_type: &HirType) -> Type {
        match hir_type {
            HirType::Named(_, name) => {
                let base = resolve_named_type(name);
                if base != Type::Unknown {
                    return base;
                }
                if self.structs.contains_key(name.as_str()) {
                    return Type::Struct(name.clone());
                }
                if self.enums.contains_key(name.as_str()) {
                    return Type::Enum(name.clone());
                }
                Type::Unknown
            }
            HirType::Array(_, inner) => Type::Array(Box::new(self.resolve_hir_type(inner))),
            HirType::Tuple(_, elems) => {
                Type::Tuple(elems.iter().map(|e| self.resolve_hir_type(e)).collect())
            }
        }
    }
}
