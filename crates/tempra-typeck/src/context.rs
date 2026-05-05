use crate::env::ScopeStack;
use crate::error::TypeError;
use crate::types::{resolve_named_type, ComptimeValue, GenericParamId, Type};
use std::collections::{HashMap, HashSet};
use tempra_ast::Visibility;
use tempra_hir::types::substitute_self;
use tempra_hir::*;
use tempra_span::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericParam {
    pub id: crate::types::GenericParamId,
    pub name: String,
    pub constraint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpecializationKey {
    /// Temporary: use function name for Phase 2 bookkeeping.
    /// Phase 3 must transition to a stable FunctionId.
    pub function_name: String,
    /// Ordered list of type arguments (T, U, etc.).
    pub type_args: Vec<Type>,
    /// Ordered list of compile-time values for 'comptime' parameters.
    /// NOTE: Runtime parameters MUST NOT enter the specialization key.
    /// NOTE: Float comptime values remain unsupported due to Hash/Eq requirements.
    pub comptime_args: Vec<ComptimeValue>,
}

#[derive(Debug, Clone)]
pub struct FunctionParam {
    pub name: String,
    pub ty: Type,
    pub is_inout: bool,
    pub kind: tempra_ast::ParameterKind,
}

/// Metadata about a function signature used during type checking.
#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub visibility: Visibility,
    pub generic_params: Vec<GenericParam>,
    pub params: Vec<FunctionParam>,
    pub return_type: Type,
    pub is_fallible: bool,
}

/// Metadata about a struct definition.
#[derive(Debug, Clone)]
pub struct StructInfo {
    pub visibility: Visibility,
    pub generic_params: Vec<GenericParam>,
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
    pub generic_params: Vec<GenericParam>,
    pub variants: HashMap<String, Vec<Type>>,
}

#[derive(Debug, Clone)]
pub struct InterfaceInfo {
    pub visibility: Visibility,
    pub methods: HashMap<String, InterfaceMethodRequirementInfo>,
}

#[derive(Debug, Clone)]
pub struct InterfaceMethodRequirementInfo {
    pub name: String,
    pub params: Vec<Type>,
    pub return_type: Type,
}

/// Global type-checking context that is built in a registration pass and
/// then queried during the checking pass.
pub struct TypeContext {
    pub functions: HashMap<String, FunctionSignature>,
    pub structs: HashMap<String, StructInfo>,
    pub enums: HashMap<String, EnumInfo>,
    pub interfaces: HashMap<String, InterfaceInfo>,
    pub variant_to_enums: HashMap<String, Vec<String>>,
    /// Methods defined in 'extend' blocks, keyed by the target type.
    pub extends: HashMap<Type, Vec<String>>,
    pub scopes: ScopeStack,
    pub errors: Vec<TypeError>,
    pub current_return_type: Option<Type>,
    pub current_is_fallible: bool,
    /// The name of the interface currently being checked, if any.
    pub current_interface: Option<String>,
    pub current_generic_params: Vec<GenericParamId>,
    pub current_generic_params_info: Vec<GenericParam>,
    pub next_generic_param_id: u32,
    pub is_comptime: bool,
    pub interpreter: Option<tempra_interpreter::interpreter::Interpreter>,
    /// Bookkeeping for specializations requested during typechecking.
    pub specialization_log: HashSet<SpecializationKey>,
}

impl TypeContext {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            interfaces: HashMap::new(),
            variant_to_enums: HashMap::new(),
            extends: HashMap::new(),
            scopes: ScopeStack::new(),
            errors: Vec::new(),
            current_return_type: None,
            current_is_fallible: false,
            current_interface: None,
            current_generic_params: Vec::new(),
            current_generic_params_info: Vec::new(),
            next_generic_param_id: 0,
            is_comptime: false,
            interpreter: None,
            specialization_log: HashSet::new(),
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
        if let Some(param) = self
            .current_generic_params_info
            .iter()
            .find(|p| p.name == name)
        {
            return Type::GenericParam {
                id: param.id,
                name: name.to_string(),
                constraint: param.constraint.clone(),
            };
        }
        if self.structs.contains_key(name) {
            return Type::Struct(name.to_string(), vec![]);
        }
        if self.enums.contains_key(name) {
            return Type::Enum(name.to_string(), vec![]);
        }
        if self.interfaces.contains_key(name) {
            return Type::Struct(name.to_string(), vec![]);
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
            HirType::Apply(_, callee, args) => {
                let base = self.resolve_hir_type(callee);
                let arg_types = args
                    .iter()
                    .map(|a| self.resolve_hir_type(a))
                    .collect::<Vec<_>>();

                match base {
                    Type::Struct(name, _) => Type::Struct(name, arg_types),
                    Type::Enum(name, _) => Type::Enum(name, arg_types),
                    Type::Option(_) => {
                        if arg_types.len() != 1 {
                            return Type::Error;
                        }
                        Type::Option(Box::new(arg_types[0].clone()))
                    }
                    Type::Result(_, _) => {
                        if arg_types.len() != 2 {
                            return Type::Error;
                        }
                        Type::Result(
                            Box::new(arg_types[0].clone()),
                            Box::new(arg_types[1].clone()),
                        )
                    }
                    _ => Type::Error,
                }
            }
            HirType::Optional(_, inner) => Type::Option(Box::new(self.resolve_hir_type(inner))),
            HirType::Result(_, ok, err) => {
                let ok_ty = self.resolve_hir_type(ok);
                let err_ty = self.resolve_hir_type(err);
                Type::Result(Box::new(ok_ty), Box::new(err_ty))
            }
            HirType::Function(_, params, ret) => {
                let p_types = params.iter().map(|p| self.resolve_hir_type(p)).collect();
                let r_type = self.resolve_hir_type(ret);
                Type::Function(p_types, Box::new(r_type))
            }
            HirType::SelfType(_span) => {
                if self.current_interface.is_none() {
                    // We don't have a direct way to emit error from here without mut?
                    // Actually, TypeContext has a way to record errors if it's mut.
                    // resolve_hir_type is &self.
                    // I'll have to use a placeholder or handle it in check_item.
                }
                Type::SelfType
            }
        }
    }
    /// Returns the list of enums that own a given variant name (for diagnostic use).
    pub fn variant_owners(&self, variant: &str) -> &[String] {
        self.variant_to_enums
            .get(variant)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Finds which enum owns a variant, using the expected type as a hint to resolve ambiguity.
    pub fn find_enum_for_variant(
        &self,
        variant: &str,
        expected: Option<&Type>,
    ) -> Result<String, String> {
        let candidates = match self.variant_to_enums.get(variant) {
            Some(c) => c,
            None => {
                return Err(format!(
                    "Unknown constructor '{}': no enum defines this variant",
                    variant
                ))
            }
        };

        if let Some(Type::Enum(expected_name, _)) = expected {
            if candidates.contains(expected_name) {
                return Ok(expected_name.clone());
            } else {
                return Err(format!(
                    "constructor '{}' does not belong to enum '{}'",
                    variant, expected_name
                ));
            }
        }

        if candidates.len() == 1 {
            let enum_name = candidates[0].clone();
            let info = &self.enums[&enum_name];
            if !info.generic_params.is_empty() && expected.is_none() {
                return Err(format!(
                    "cannot infer generic enum type for constructor '{}'; add a type annotation",
                    variant
                ));
            }
            Ok(enum_name)
        } else {
            let mut sorted = candidates.clone();
            sorted.sort();
            let list = sorted.join(", ");
            Err(format!(
                "ambiguous constructor '{}' — candidates: {}. Add a type annotation to disambiguate.",
                variant, list
            ))
        }
    }

    pub fn lookup_field(&self, struct_name: &str, field_name: &str) -> Option<Type> {
        self.structs
            .get(struct_name)
            .and_then(|info| info.fields.get(field_name))
            .map(|f| f.ty.clone())
    }

    /// Centralized method lookup for Phase 6.
    /// Returns the signature of a method if found on the type or in an 'extend' block.
    pub fn lookup_method_signature(
        &self,
        receiver_ty: &Type,
        method_name: &str,
    ) -> Option<FunctionSignature> {
        // 1. Check if it's a struct and the method is defined as a function with name "StructName.methodName"
        // In Tempra, methods are currently just functions named "Type.method" or similar?
        // Wait, check_expr.rs check_function_call uses ctx.functions.get(func_name).

        // Actually, Tempra v0.1 usually models methods as "TypeName.methodName" in the global function map
        // for functions defined inside the type or in extend blocks.

        let type_name = match receiver_ty {
            Type::Struct(name, _) => Some(name.clone()),
            Type::Enum(name, _) => Some(name.clone()),
            Type::GenericParam { id, .. } => {
                // Find the constraint for this generic parameter
                let constraint = self
                    .current_generic_params_info
                    .iter()
                    .find(|p| p.id == *id)
                    .and_then(|p| p.constraint.clone());

                if let Some(interface_name) = constraint {
                    if let Some(interface) = self.interfaces.get(&interface_name) {
                        if let Some(req) = interface.methods.get(method_name) {
                            return Some(FunctionSignature {
                                visibility: tempra_ast::Visibility::Public,
                                generic_params: Vec::new(),
                                params: req
                                    .params
                                    .iter()
                                    .map(|p| crate::context::FunctionParam {
                                        name: "self".to_string(), // approximation
                                        ty: substitute_self(p, receiver_ty),
                                        is_inout: false,
                                        kind: tempra_ast::ParameterKind::Runtime,
                                    })
                                    .collect(),
                                return_type: substitute_self(&req.return_type, receiver_ty),
                                is_fallible: false,
                            });
                        }
                    }
                }
                None
            }
            _ => None,
        };
        if let Some(name) = &type_name {
            // Check if it's a function "Type.method"
            let mangled = format!("{}.{}", name, method_name);
            if let Some(sig) = self.functions.get(&mangled) {
                let mut sig = sig.clone();
                for p in &mut sig.params {
                    p.ty = substitute_self(&p.ty, receiver_ty);
                }
                sig.return_type = substitute_self(&sig.return_type, receiver_ty);
                return Some(sig);
            }

            // Check if it's an interface method (rare but possible if a type is named after an interface)
            if let Some(interface) = self.interfaces.get(name) {
                if let Some(req) = interface.methods.get(method_name) {
                    return Some(FunctionSignature {
                        visibility: interface.visibility,
                        generic_params: Vec::new(),
                        params: req
                            .params
                            .iter()
                            .enumerate()
                            .map(|(i, p_ty)| crate::context::FunctionParam {
                                name: if i == 0 {
                                    "self".to_string()
                                } else {
                                    format!("p{}", i)
                                },
                                ty: substitute_self(p_ty, receiver_ty),
                                is_inout: false,
                                kind: tempra_ast::ParameterKind::Runtime,
                            })
                            .collect(),
                        return_type: substitute_self(&req.return_type, receiver_ty),
                        is_fallible: false,
                    });
                }
            }
        }

        // 2. Check explicitly registered 'extends' (if we use a different mangling or storage)
        if let Some(methods) = self.extends.get(receiver_ty) {
            for m_name in methods {
                if let Some(idx) = m_name.rfind('.') {
                    let actual_name = &m_name[idx + 1..];
                    if actual_name == method_name {
                        if let Some(sig) = self.functions.get(m_name) {
                            // Phase 6: Substitute Self with the actual receiver type
                            let mut sig = sig.clone();
                            for p in &mut sig.params {
                                p.ty = substitute_self(&p.ty, receiver_ty);
                            }
                            sig.return_type = substitute_self(&sig.return_type, receiver_ty);
                            return Some(sig);
                        }
                    }
                }
            }
        }

        None
    }
}

impl TypeContext {
    pub fn verify_not_interface(&mut self, ty: &Type, span: tempra_span::Span) {
        if let Type::Struct(name, _) = ty {
            if self.interfaces.contains_key(name) {
                self.error(
                    format!(
                        "interfaces are static constraints and cannot be used as runtime types"
                    ),
                    span,
                );
            }
        } else if let Type::Enum(name, _) = ty {
            if self.interfaces.contains_key(name) {
                self.error(
                    format!(
                        "interfaces are static constraints and cannot be used as runtime types"
                    ),
                    span,
                );
            }
        }
    }

    pub fn verify_no_self(&mut self, ty: &Type, span: tempra_span::Span) {
        if ty.contains_self() {
            self.error(
                format!("'Self' is only allowed within interface requirements"),
                span,
            );
        }
    }
}
