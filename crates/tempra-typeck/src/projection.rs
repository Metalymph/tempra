use crate::context::TypeContext;
use crate::types::{GenericParamId, Type};
use std::collections::{HashMap, HashSet};

pub struct ProjectionEnv<'a> {
    pub generic_subst: &'a HashMap<GenericParamId, Type>,
    pub associated_bindings: &'a HashMap<(GenericParamId, String), Type>,
}

pub fn normalize_type_projections(ty: &Type, env: &ProjectionEnv, ctx: &mut TypeContext) -> Type {
    let mut visited = HashSet::new();
    normalize_recursive(ty, env, ctx, &mut visited)
}

fn normalize_recursive(
    ty: &Type,
    env: &ProjectionEnv,
    ctx: &mut TypeContext,
    visited: &mut HashSet<(GenericParamId, String)>,
) -> Type {
    match ty {
        Type::Projection { base, assoc } => {
            let key = (*base, assoc.clone());
            if visited.contains(&key) {
                return Type::Error;
            }

            if let Some(resolved) = env.associated_bindings.get(&key) {
                visited.insert(key.clone());
                let res = normalize_recursive(resolved, env, ctx, visited);
                visited.remove(&key);
                return res;
            }

            if let Some(concrete_base) = env.generic_subst.get(base).cloned() {
                let itf_candidates: Vec<String> = ctx
                    .interfaces
                    .iter()
                    .filter(|(_, itf)| itf.associated_types.contains(assoc))
                    .map(|(name, _)| name.clone())
                    .collect();

                for itf_name in itf_candidates {
                    if crate::check_expr::satisfies_interface_no_errors(
                        ctx,
                        &concrete_base,
                        &itf_name,
                    ) {
                        if let Some(mappings) =
                            ctx.infer_associated_types(&concrete_base, &itf_name)
                        {
                            if let Some(resolved) = mappings.get(assoc) {
                                visited.insert(key.clone());
                                let res = normalize_recursive(resolved, env, ctx, visited);
                                visited.remove(&key);
                                return res;
                            }
                        }
                    }
                }
            }

            ty.clone()
        }
        Type::GenericParam { id, .. } => {
            if let Some(res) = env.generic_subst.get(id) {
                res.clone()
            } else {
                ty.clone()
            }
        }
        Type::Array(inner) => Type::Array(Box::new(normalize_recursive(inner, env, ctx, visited))),
        Type::Slice(inner) => Type::Slice(Box::new(normalize_recursive(inner, env, ctx, visited))),
        Type::Option(inner) => {
            Type::Option(Box::new(normalize_recursive(inner, env, ctx, visited)))
        }
        Type::Result(ok, err) => Type::Result(
            Box::new(normalize_recursive(ok, env, ctx, visited)),
            Box::new(normalize_recursive(err, env, ctx, visited)),
        ),
        Type::Tuple(elems) => Type::Tuple(
            elems
                .iter()
                .map(|e| normalize_recursive(e, env, ctx, visited))
                .collect(),
        ),
        Type::Function(params, ret) => Type::Function(
            params
                .iter()
                .map(|p| normalize_recursive(p, env, ctx, visited))
                .collect(),
            Box::new(normalize_recursive(ret, env, ctx, visited)),
        ),
        Type::Struct(name, args) => Type::Struct(
            name.clone(),
            args.iter()
                .map(|a| normalize_recursive(a, env, ctx, visited))
                .collect(),
        ),
        Type::Enum(name, args) => Type::Enum(
            name.clone(),
            args.iter()
                .map(|a| normalize_recursive(a, env, ctx, visited))
                .collect(),
        ),
        Type::Meta(inner) => Type::Meta(Box::new(normalize_recursive(inner, env, ctx, visited))),
        _ => ty.clone(),
    }
}
