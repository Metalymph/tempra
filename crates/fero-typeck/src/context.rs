use crate::env::ScopeStack;
use crate::error::TypeError;
use crate::types::{resolve_named_type, Type};
use fero_ast::Visibility;
use fero_hir::*;
use fero_span::Span;
use std::collections::HashMap;

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
    pub variant_to_enums: HashMap<String, Vec<String>>,
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
            variant_to_enums: HashMap::new(),
            scopes: ScopeStack::new(),
            errors: Vec::new(),
            current_return_type: None,
            current_is_fallible: false,
        }
    }

    pub fn error(&mut self, message: impl Into<String>, span: Span) {
        self.errors.push(TypeError::new(message, span));
    }

    /// Resolves a simple type name into a semantic Type, checking builtins then user types.
    pub fn resolve_type_name(&self, name: &str) -> Type {
        let base = resolve_named_type(name);
        if base != Type::Unknown {
            return base;
        }
        if self.structs.contains_key(name) {
            return Type::Struct(name.to_string());
        }
        if self.enums.contains_key(name) {
            return Type::Enum(name.to_string());
        }
        Type::Unknown
    }

    /// Resolve an HIR type node into a semantic Type.
    pub fn resolve_hir_type(&self, hir_type: &HirType) -> Type {
        match hir_type {
            HirType::Named(_, name) => self.resolve_type_name(name),
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
                if name == "Option" {
                    if args.len() != 1 {
                        return Type::Error;
                    }
                    let inner = self.resolve_hir_type(&args[0]);
                    return Type::Option(Box::new(inner));
                }
                Type::Unknown
            }
            HirType::Option(_, inner) => Type::Option(Box::new(self.resolve_hir_type(inner))),
            HirType::Result(_, ok, err) => {
                let ok_ty = self.resolve_hir_type(ok);
                let err_ty = err
                    .as_ref()
                    .map(|e| self.resolve_hir_type(e))
                    .unwrap_or(Type::BuiltinError);
                Type::Result(Box::new(ok_ty), Box::new(err_ty))
            }
        }
    }
    /// Finds which enum owns a variant, using the expected type as a hint to resolve ambiguity.
    pub fn find_enum_for_variant(&self, variant: &str, expected: Option<&Type>) -> Result<String, String> {
        let candidates = match self.variant_to_enums.get(variant) {
            Some(c) => c,
            None => return Err(format!("Unknown constructor '{}'", variant)),
        };

        if let Some(Type::Enum(expected_name)) = expected {
            if candidates.contains(expected_name) {
                return Ok(expected_name.clone());
            }
        }

        if candidates.len() == 1 {
            Ok(candidates[0].clone())
        } else {
            Err(format!(
                "ambiguous constructor '{}'. Multiple enums found: {}. Use a type hint to resolve this.",
                variant,
                candidates.join(", ")
            ))
        }
    }
}
