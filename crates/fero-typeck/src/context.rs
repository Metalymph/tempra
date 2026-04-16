use std::collections::HashMap;
use fero_span::Span;
use fero_hir::*;
use crate::types::{Type, resolve_named_type};
use crate::error::TypeError;
use crate::env::ScopeStack;
use fero_ast::Visibility;

/// Metadata about a function signature used during type checking.
#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub visibility: Visibility,
    pub params: Vec<(String, Type)>,
    pub return_type: Type,
    pub is_fallible: bool,
}

/// Metadata about a struct definition.
#[derive(Debug, Clone)]
pub struct StructInfo {
    pub visibility: Visibility,
    pub fields: HashMap<String, StructFieldInfo>,
    pub field_order: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct StructFieldInfo {
    pub visibility: Visibility,
    pub ty: Type,
    pub is_mutable: bool,
    pub has_default: bool,
}

/// Metadata about an enum definition.
#[derive(Debug, Clone)]
pub struct EnumInfo {
    pub visibility: Visibility,
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
    pub current_is_fallible: bool,
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
            current_is_fallible: false,
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
            HirType::Generic(_, name, args) => {
                if name == "Result" {
                    if args.len() != 2 {
                         // Should ideally be an error, but let's stick to Type::Error for now
                         return Type::Error;
                    }
                    let ok = self.resolve_hir_type(&args[0]);
                    let err = self.resolve_hir_type(&args[1]);
                    return Type::Result(Box::new(ok), Box::new(err));
                }
                Type::Unknown
            }
        }
    }
}
