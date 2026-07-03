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
    pub name: String,
    pub visibility: Visibility,
    pub methods: HashMap<String, InterfaceMethodRequirementInfo>,
    pub associated_types: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct InterfaceMethodRequirementInfo {
    pub name: String,
    pub params: Vec<Type>,
    pub param_inout: Vec<bool>,
    pub return_type: Type,
}

/// Global type-checking context that is built in a registration pass and
/// then queried during the checking pass.
#[derive(Clone)]
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
    /// Collected typecheck facts.
    pub facts: crate::facts::TypecheckFacts,
    /// Helper declaration span maps.
    pub struct_field_spans: HashMap<(String, String), Span>,
    pub enum_variant_spans: HashMap<(String, String), Span>,
    pub method_spans: HashMap<(String, String), Span>,
    pub function_spans: HashMap<String, Span>,
    pub source_text: Option<String>,
    pub current_module: String,
    pub imports: HashMap<String, String>,
    pub modules: HashSet<String>,
    pub is_single_file_fallback: bool,
    pub module_facts: HashMap<String, crate::facts::TypecheckFacts>,
}

impl TypeContext {
    pub fn new() -> Self {
        let mut ctx = Self {
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
            facts: crate::facts::TypecheckFacts::default(),
            struct_field_spans: HashMap::new(),
            enum_variant_spans: HashMap::new(),
            method_spans: HashMap::new(),
            function_spans: HashMap::new(),
            source_text: None,
            current_module: String::new(),
            imports: HashMap::new(),
            modules: HashSet::new(),
            is_single_file_fallback: false,
            module_facts: HashMap::new(),
        };

        ctx.register_builtins();
        ctx
    }

    pub fn is_module(&self, name: &str) -> bool {
        if self.modules.contains(name) || self.imports.contains_key(name) {
            return true;
        }
        let dot_prefix = format!("{}.", name);
        for m in &self.modules {
            if m.starts_with(&dot_prefix) {
                return true;
            }
        }
        for k in self.imports.keys() {
            if k.starts_with(&dot_prefix) {
                return true;
            }
        }
        false
    }

    pub fn find_name_span(&self, name: &str, span: Span) -> Span {
        if let Some(source) = &self.source_text {
            let start = span.start as usize;
            let end = span.end as usize;
            if start < source.len() && end <= source.len() {
                let sub = &source[start..end];
                if let Some(pos) = sub.find(name) {
                    let s = span.start + pos as u32;
                    return Span::new(s, s + name.len() as u32);
                }
            }
        }
        Span::new(span.start, span.start + name.len() as u32)
    }

    pub fn register_builtins(&mut self) {
        let mut collection = InterfaceInfo {
            name: "Collection".to_string(),
            visibility: Visibility::Public,
            methods: HashMap::new(),
            associated_types: vec!["Item".to_string()],
        };
        collection.methods.insert(
            "len".to_string(),
            InterfaceMethodRequirementInfo {
                name: "len".to_string(),
                params: vec![Type::SelfType],
                param_inout: vec![false],
                return_type: Type::Int,
            },
        );
        collection.methods.insert(
            "is_empty".to_string(),
            InterfaceMethodRequirementInfo {
                name: "is_empty".to_string(),
                params: vec![Type::SelfType],
                param_inout: vec![false],
                return_type: Type::Bool,
            },
        );
        self.interfaces.insert("Collection".to_string(), collection);

        let mut iterator = InterfaceInfo {
            name: "Iterator".to_string(),
            visibility: Visibility::Public,
            methods: HashMap::new(),
            associated_types: vec!["Item".to_string()],
        };
        iterator.methods.insert(
            "next".to_string(),
            InterfaceMethodRequirementInfo {
                name: "next".to_string(),
                params: vec![Type::SelfType],
                param_inout: vec![true],
                return_type: Type::Option(Box::new(Type::Associated(
                    "Iterator".to_string(),
                    "Item".to_string(),
                ))),
            },
        );
        self.interfaces.insert("Iterator".to_string(), iterator);

        // Iterable: types that can produce an Iterator via iter()
        let mut iterable = InterfaceInfo {
            name: "Iterable".to_string(),
            visibility: Visibility::Public,
            methods: HashMap::new(),
            associated_types: vec!["Item".to_string(), "Iter".to_string()],
        };
        iterable.methods.insert(
            "iter".to_string(),
            InterfaceMethodRequirementInfo {
                name: "iter".to_string(),
                params: vec![Type::SelfType],
                param_inout: vec![false],
                return_type: Type::Associated("Iterable".to_string(), "Iter".to_string()),
            },
        );
        self.interfaces.insert("Iterable".to_string(), iterable);
    }

    pub fn error(&mut self, message: impl Into<String>, span: Span) {
        let mut err = TypeError::new(message, span);
        if !self.current_module.is_empty() {
            err.module_name = Some(self.current_module.clone());
        }
        self.errors.push(err);
    }

    /// Resolves a simple type name into a semantic Type, checking builtins then user types.
    pub fn resolve_type_name(&self, name: &str) -> Type {
        let base = resolve_named_type(name);
        if base != Type::Unknown {
            return base;
        }

        // Associated type lookup
        if let Some(itf_name) = &self.current_interface {
            if let Some(itf) = self.interfaces.get(itf_name) {
                if itf.associated_types.contains(&name.to_string()) {
                    return Type::Associated(itf_name.clone(), name.to_string());
                }
            }
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

        if self.is_module(name) {
            return Type::Namespace(name.to_string());
        }

        if !self.current_module.is_empty() {
            let qualified = format!("{}.{}", self.current_module, name);
            if self.structs.contains_key(&qualified) {
                return Type::Struct(qualified, vec![]);
            }
            if self.enums.contains_key(&qualified) {
                return Type::Enum(qualified, vec![]);
            }
            if self.interfaces.contains_key(&qualified) {
                return Type::Interface(qualified);
            }
        }

        if self.structs.contains_key(name) {
            return Type::Struct(name.to_string(), vec![]);
        }
        if self.enums.contains_key(name) {
            return Type::Enum(name.to_string(), vec![]);
        }
        if self.interfaces.contains_key(name) {
            return Type::Interface(name.to_string());
        }
        Type::Unknown
    }

    /// Resolve an HIR type node into a semantic Type.
    pub fn resolve_hir_type(&mut self, hir_type: &HirType) -> Type {
        match hir_type {
            HirType::Named(_, name) => self.resolve_type_name(name),
            HirType::Array(_, inner) => Type::Array(Box::new(self.resolve_hir_type(inner))),
            HirType::Tuple(_, elems) => {
                let mut resolved = Vec::new();
                for e in elems {
                    resolved.push(self.resolve_hir_type(e));
                }
                Type::Tuple(resolved)
            }
            HirType::Apply(_, callee, args) => {
                let base = self.resolve_hir_type(callee);
                let mut arg_types = Vec::new();
                for a in args {
                    arg_types.push(self.resolve_hir_type(a));
                }

                match base {
                    Type::Struct(name, _) => Type::Struct(name, arg_types),
                    Type::Enum(name, _) => Type::Enum(name, arg_types),
                    Type::Option(_) => {
                        if arg_types.len() != 1 {
                            return Type::Error;
                        }
                        Type::Option(Box::new(arg_types[0].clone()))
                    }
                    Type::Array(_) => {
                        if arg_types.len() != 1 {
                            return Type::Error;
                        }
                        Type::Array(Box::new(arg_types[0].clone()))
                    }
                    Type::Slice(_) => {
                        if arg_types.len() != 1 {
                            return Type::Error;
                        }
                        Type::Slice(Box::new(arg_types[0].clone()))
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
                let mut p_types = Vec::new();
                for p in params {
                    p_types.push(self.resolve_hir_type(p));
                }
                let r_type = self.resolve_hir_type(ret);
                Type::Function(p_types, Box::new(r_type))
            }
            HirType::SelfType(_span) => Type::SelfType,
            HirType::Projection(span, base_hir, assoc) => {
                let base_ty = self.resolve_hir_type(base_hir);
                if let Type::Namespace(ns) = &base_ty {
                    let qualified = format!("{}.{}", ns, assoc);
                    if self.is_module(&qualified) {
                        return Type::Namespace(qualified);
                    }
                    if self.structs.contains_key(&qualified) {
                        let info = &self.structs[&qualified];
                        if info.visibility == Visibility::Private && self.current_module != *ns {
                            self.error(
                                format!(
                                    "cannot access private struct '{}' of module '{}'",
                                    assoc, ns
                                ),
                                *span,
                            );
                        }
                        if let Some(&def_span) = self.function_spans.get(&qualified) {
                            let assoc_span = self.find_name_span(assoc, *span);
                            self.facts
                                .cross_file_resolutions
                                .insert(assoc_span, (ns.clone(), def_span));
                            self.facts.resolved_symbols.insert(assoc_span, def_span);
                        }
                        return Type::Struct(qualified, vec![]);
                    }
                    if self.enums.contains_key(&qualified) {
                        let info = &self.enums[&qualified];
                        if info.visibility == Visibility::Private && self.current_module != *ns {
                            self.error(
                                format!(
                                    "cannot access private enum '{}' of module '{}'",
                                    assoc, ns
                                ),
                                *span,
                            );
                        }
                        if let Some(&def_span) = self.function_spans.get(&qualified) {
                            let assoc_span = self.find_name_span(assoc, *span);
                            self.facts
                                .cross_file_resolutions
                                .insert(assoc_span, (ns.clone(), def_span));
                            self.facts.resolved_symbols.insert(assoc_span, def_span);
                        }
                        return Type::Enum(qualified, vec![]);
                    }
                    if self.interfaces.contains_key(&qualified) {
                        let info = &self.interfaces[&qualified];
                        if info.visibility == Visibility::Private && self.current_module != *ns {
                            self.error(
                                format!(
                                    "cannot access private interface '{}' of module '{}'",
                                    assoc, ns
                                ),
                                *span,
                            );
                        }
                        if let Some(&def_span) = self.function_spans.get(&qualified) {
                            let assoc_span = self.find_name_span(assoc, *span);
                            self.facts
                                .cross_file_resolutions
                                .insert(assoc_span, (ns.clone(), def_span));
                            self.facts.resolved_symbols.insert(assoc_span, def_span);
                        }
                        return Type::Interface(qualified);
                    }
                    if self.is_single_file_fallback {
                        return Type::Unknown;
                    }
                    self.error(
                        format!("no type '{}' found in module '{}'", assoc, ns),
                        *span,
                    );
                    return Type::Error;
                }
                if let Type::GenericParam {
                    id,
                    name,
                    constraint,
                } = base_ty
                {
                    if let Some(itf_name) = constraint {
                        if let Some(itf) = self.interfaces.get(&itf_name) {
                            if itf.associated_types.contains(assoc) {
                                return Type::Projection {
                                    base: id,
                                    assoc: assoc.clone(),
                                };
                            } else {
                                self.error(format!("generic parameter {} has no interface constraint declaring associated type {}", name, assoc), *span);
                            }
                        }
                    } else {
                        self.error(format!("associated type projection is only valid on constrained generic parameters"), *span);
                    }
                } else if base_ty != Type::Error {
                    self.error(format!("associated type projection is only valid on constrained generic parameters"), *span);
                }
                Type::Error
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
        &mut self,
        receiver_ty: &Type,
        method_name: &str,
    ) -> Option<FunctionSignature> {
        // 1. Check direct methods and explicitly registered 'extends'
        if let Some(sig) = self.lookup_direct_method_signature(receiver_ty, method_name) {
            return Some(sig);
        }

        // 2. Check interface constraints for generic parameters
        let mut generic_info = None;
        if let Type::GenericParam { id, .. } = receiver_ty {
            if let Some(constraint) = self
                .current_generic_params_info
                .iter()
                .find(|p| p.id == *id)
                .and_then(|p| p.constraint.clone())
            {
                generic_info = Some((*id, constraint));
            }
        }

        if let Some((id, itf_name)) = generic_info {
            if let Some(itf) = self.interfaces.get(&itf_name) {
                if let Some(req) = itf.methods.get(method_name) {
                    return Some(FunctionSignature {
                        visibility: itf.visibility,
                        generic_params: Vec::new(),
                        params: req
                            .params
                            .iter()
                            .enumerate()
                            .map(|(i, p_ty)| FunctionParam {
                                name: if i == 0 {
                                    "self".to_string()
                                } else {
                                    format!("p{}", i)
                                },
                                ty: self.resolve_associated_to_projections(
                                    &substitute_self(p_ty, receiver_ty),
                                    id,
                                ),
                                is_inout: req.param_inout[i],
                                kind: ParameterKind::Runtime,
                            })
                            .collect(),
                        return_type: self.resolve_associated_to_projections(
                            &substitute_self(&req.return_type, receiver_ty),
                            id,
                        ),
                        is_fallible: false,
                    });
                }
            }
        }

        // 3. Check if concrete type satisfies an interface with this method (Projection)
        let itf_names: Vec<String> = self.interfaces.keys().cloned().collect();
        for itf_name in itf_names {
            let (has_method, visibility) = {
                let itf = &self.interfaces[&itf_name];
                (itf.methods.contains_key(method_name), itf.visibility)
            };
            if has_method {
                if crate::check_expr::satisfies_interface_no_errors(self, receiver_ty, &itf_name) {
                    let req = self.interfaces[&itf_name]
                        .methods
                        .get(method_name)
                        .cloned()
                        .unwrap();
                    if let Some(mappings) = self.infer_associated_types(receiver_ty, &itf_name) {
                        return Some(FunctionSignature {
                            visibility,
                            generic_params: Vec::new(),
                            params: req
                                .params
                                .iter()
                                .enumerate()
                                .map(|(i, p_ty)| FunctionParam {
                                    name: if i == 0 {
                                        "self".to_string()
                                    } else {
                                        format!("p{}", i)
                                    },
                                    ty: self.resolve_associated_types(
                                        &substitute_self(p_ty, receiver_ty),
                                        &mappings,
                                    ),
                                    is_inout: req.param_inout[i],
                                    kind: ParameterKind::Runtime,
                                })
                                .collect(),
                            return_type: self.resolve_associated_types(
                                &substitute_self(&req.return_type, receiver_ty),
                                &mappings,
                            ),
                            is_fallible: false,
                        });
                    }
                }
            }
        }

        None
    }

    pub fn lookup_direct_method_signature(
        &self,
        receiver_ty: &Type,
        method_name: &str,
    ) -> Option<FunctionSignature> {
        // 1. Check direct methods
        let full_name = format!("{}.{}", receiver_ty.get_name(), method_name);
        if let Some(sig) = self.functions.get(&full_name) {
            let mut sig = sig.clone();
            for p in &mut sig.params {
                p.ty = substitute_self(&p.ty, receiver_ty);
            }
            sig.return_type = substitute_self(&sig.return_type, receiver_ty);
            return Some(sig);
        }

        // 2. Check extends
        if let Some(methods) = self.extends.get(receiver_ty) {
            for m_name in methods {
                if m_name.ends_with(&format!(".{}", method_name)) {
                    if let Some(sig) = self.functions.get(m_name) {
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
        None
    }

    pub fn infer_associated_types(
        &self,
        ty: &Type,
        interface_name: &str,
    ) -> Option<HashMap<String, Type>> {
        if interface_name == "Collection" {
            match ty {
                Type::Array(inner) | Type::Slice(inner) => {
                    let mut mappings = HashMap::new();
                    mappings.insert("Item".to_string(), (**inner).clone());
                    return Some(mappings);
                }
                _ => {}
            }
        }
        if interface_name == "Iterator" {
            match ty {
                Type::ArrayIter(inner) | Type::SliceIter(inner) => {
                    let mut mappings = HashMap::new();
                    mappings.insert("Item".to_string(), (**inner).clone());
                    return Some(mappings);
                }
                _ => {}
            }
        }
        if interface_name == "Iterable" {
            match ty {
                Type::Array(inner) => {
                    let mut mappings = HashMap::new();
                    mappings.insert("Item".to_string(), (**inner).clone());
                    mappings.insert("Iter".to_string(), Type::ArrayIter(inner.clone()));
                    return Some(mappings);
                }
                Type::Slice(inner) => {
                    let mut mappings = HashMap::new();
                    mappings.insert("Item".to_string(), (**inner).clone());
                    mappings.insert("Iter".to_string(), Type::SliceIter(inner.clone()));
                    return Some(mappings);
                }
                Type::String => {
                    let mut mappings = HashMap::new();
                    mappings.insert("Item".to_string(), Type::UInt8);
                    mappings.insert("Iter".to_string(), Type::SliceIter(Box::new(Type::UInt8)));
                    return Some(mappings);
                }
                _ => {}
            }
        }

        if let Type::GenericParam { id, constraint, .. } = ty {
            if let Some(c) = constraint {
                if c == interface_name {
                    let mut mappings = HashMap::new();
                    if let Some(itf) = self.interfaces.get(c) {
                        for assoc in &itf.associated_types {
                            mappings.insert(
                                assoc.clone(),
                                Type::Projection {
                                    base: *id,
                                    assoc: assoc.clone(),
                                },
                            );
                        }
                    }
                    return Some(mappings);
                }
            }
        }
        if let Type::Interface(name) = ty {
            if name == interface_name {
                let mut mappings = HashMap::new();
                if let Some(itf) = self.interfaces.get(name) {
                    for assoc in &itf.associated_types {
                        mappings
                            .insert(assoc.clone(), Type::Associated(name.clone(), assoc.clone()));
                    }
                }
                return Some(mappings);
            }
        }

        let interface = self.interfaces.get(interface_name)?.clone();
        let mut mappings = HashMap::new();
        for (method_name, req) in &interface.methods {
            let full_name = format!("{}.{}", ty.get_name(), method_name);
            if let Some(sig) = self.functions.get(&full_name) {
                let req_params: Vec<Type> =
                    req.params.iter().map(|p| substitute_self(p, ty)).collect();
                let req_ret = substitute_self(&req.return_type, ty);

                for (p_req, p_act) in req_params.iter().zip(sig.params.iter()) {
                    self.unify_associated_internal(p_req, &p_act.ty, interface_name, &mut mappings);
                }
                self.unify_associated_internal(
                    &req_ret,
                    &sig.return_type,
                    interface_name,
                    &mut mappings,
                );
            }
        }

        // Special case for Iterable: Item is often not directly in the iter() signature,
        // but it is the Item of the returned Iter (which must satisfy Iterator).
        if interface_name == "Iterable" && !mappings.contains_key("Item") {
            if let Some(iter_ty) = mappings.get("Iter") {
                if let Some(it_mappings) = self.infer_associated_types(iter_ty, "Iterator") {
                    if let Some(item_ty) = it_mappings.get("Item") {
                        mappings.insert("Item".to_string(), item_ty.clone());
                    }
                }
            }
        }

        if interface
            .associated_types
            .iter()
            .all(|name| mappings.contains_key(name))
        {
            Some(mappings)
        } else {
            None
        }
    }

    fn unify_associated_internal(
        &self,
        req: &Type,
        actual: &Type,
        interface_name: &str,
        mappings: &mut HashMap<String, Type>,
    ) {
        match req {
            Type::Associated(itf, name) if itf == interface_name => {
                mappings
                    .entry(name.clone())
                    .or_insert_with(|| actual.clone());
            }
            Type::Array(inner) => {
                if let Type::Array(act_inner) = actual {
                    self.unify_associated_internal(inner, act_inner, interface_name, mappings);
                }
            }
            Type::Slice(inner) => {
                if let Type::Slice(act_inner) = actual {
                    self.unify_associated_internal(inner, act_inner, interface_name, mappings);
                }
            }
            Type::Option(inner) => {
                if let Type::Option(act_inner) = actual {
                    self.unify_associated_internal(inner, act_inner, interface_name, mappings);
                }
            }
            Type::Result(ok, err) => {
                if let Type::Result(act_ok, act_err) = actual {
                    self.unify_associated_internal(ok, act_ok, interface_name, mappings);
                    self.unify_associated_internal(err, act_err, interface_name, mappings);
                }
            }
            Type::Tuple(elems) => {
                if let Type::Tuple(act_elems) = actual {
                    if elems.len() == act_elems.len() {
                        for (e, ae) in elems.iter().zip(act_elems.iter()) {
                            self.unify_associated_internal(e, ae, interface_name, mappings);
                        }
                    }
                }
            }
            Type::Function(params, ret) => {
                if let Type::Function(act_params, act_ret) = actual {
                    if params.len() == act_params.len() {
                        for (p, ap) in params.iter().zip(act_params.iter()) {
                            self.unify_associated_internal(p, ap, interface_name, mappings);
                        }
                        self.unify_associated_internal(ret, act_ret, interface_name, mappings);
                    }
                }
            }
            Type::Struct(name, args) => {
                if let Type::Struct(act_name, act_args) = actual {
                    if name == act_name && args.len() == act_args.len() {
                        for (a, aa) in args.iter().zip(act_args.iter()) {
                            self.unify_associated_internal(a, aa, interface_name, mappings);
                        }
                    }
                }
            }
            Type::Enum(name, args) => {
                if let Type::Enum(act_name, act_args) = actual {
                    if name == act_name && args.len() == act_args.len() {
                        for (a, aa) in args.iter().zip(act_args.iter()) {
                            self.unify_associated_internal(a, aa, interface_name, mappings);
                        }
                    }
                }
            }
            Type::Meta(inner) => {
                if let Type::Meta(act_inner) = actual {
                    self.unify_associated_internal(inner, act_inner, interface_name, mappings);
                }
            }
            _ => {}
        }
    }

    pub fn resolve_associated_types(&self, ty: &Type, mappings: &HashMap<String, Type>) -> Type {
        match ty {
            Type::Associated(_, name) => mappings.get(name).cloned().unwrap_or(ty.clone()),
            Type::Array(inner) => {
                Type::Array(Box::new(self.resolve_associated_types(inner, mappings)))
            }
            Type::Slice(inner) => {
                Type::Slice(Box::new(self.resolve_associated_types(inner, mappings)))
            }
            Type::Option(inner) => {
                Type::Option(Box::new(self.resolve_associated_types(inner, mappings)))
            }
            Type::Result(ok, err) => Type::Result(
                Box::new(self.resolve_associated_types(ok, mappings)),
                Box::new(self.resolve_associated_types(err, mappings)),
            ),
            Type::Tuple(elems) => Type::Tuple(
                elems
                    .iter()
                    .map(|e| self.resolve_associated_types(e, mappings))
                    .collect(),
            ),
            Type::Function(params, ret) => Type::Function(
                params
                    .iter()
                    .map(|p| self.resolve_associated_types(p, mappings))
                    .collect(),
                Box::new(self.resolve_associated_types(ret, mappings)),
            ),
            Type::Struct(name, args) => Type::Struct(
                name.clone(),
                args.iter()
                    .map(|a| self.resolve_associated_types(a, mappings))
                    .collect(),
            ),
            Type::Enum(name, args) => Type::Enum(
                name.clone(),
                args.iter()
                    .map(|a| self.resolve_associated_types(a, mappings))
                    .collect(),
            ),
            Type::Meta(inner) => {
                Type::Meta(Box::new(self.resolve_associated_types(inner, mappings)))
            }
            _ => ty.clone(),
        }
    }

    pub fn resolve_associated_to_projections(
        &self,
        ty: &Type,
        base_id: crate::types::GenericParamId,
    ) -> Type {
        match ty {
            Type::Associated(_, name) => Type::Projection {
                base: base_id,
                assoc: name.clone(),
            },
            Type::Array(inner) => Type::Array(Box::new(
                self.resolve_associated_to_projections(inner, base_id),
            )),
            Type::Slice(inner) => Type::Slice(Box::new(
                self.resolve_associated_to_projections(inner, base_id),
            )),
            Type::Option(inner) => Type::Option(Box::new(
                self.resolve_associated_to_projections(inner, base_id),
            )),
            Type::Result(ok, err) => Type::Result(
                Box::new(self.resolve_associated_to_projections(ok, base_id)),
                Box::new(self.resolve_associated_to_projections(err, base_id)),
            ),
            Type::Tuple(elems) => Type::Tuple(
                elems
                    .iter()
                    .map(|e| self.resolve_associated_to_projections(e, base_id))
                    .collect(),
            ),
            Type::Function(params, ret) => Type::Function(
                params
                    .iter()
                    .map(|p| self.resolve_associated_to_projections(p, base_id))
                    .collect(),
                Box::new(self.resolve_associated_to_projections(ret, base_id)),
            ),
            Type::Struct(name, args) => Type::Struct(
                name.clone(),
                args.iter()
                    .map(|a| self.resolve_associated_to_projections(a, base_id))
                    .collect(),
            ),
            Type::Enum(name, args) => Type::Enum(
                name.clone(),
                args.iter()
                    .map(|a| self.resolve_associated_to_projections(a, base_id))
                    .collect(),
            ),
            Type::Meta(inner) => Type::Meta(Box::new(
                self.resolve_associated_to_projections(inner, base_id),
            )),
            _ => ty.clone(),
        }
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
        } else if let Type::Interface(_) = ty {
            self.error(
                format!("interfaces are static constraints and cannot be used as runtime types"),
                span,
            );
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
