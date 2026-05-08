pub(crate) fn substitute_generics(
    ctx: &mut TypeContext,
    ty: &Type,
    map: &std::collections::HashMap<crate::types::GenericParamId, Type>,
) -> Type {
    let empty_bindings = std::collections::HashMap::new();
    let env = crate::projection::ProjectionEnv {
        generic_subst: map,
        associated_bindings: &empty_bindings,
    };
    crate::projection::normalize_type_projections(ty, &env, ctx)
}
