use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn project(
    local: &hir::concrete::Module,
    source: hir::concrete::FunctionId,
    input: &mir::ConeMirInput,
    callable: PersistentGeneratedCallableId,
    key: &GeneratedCallableKey,
    identities: &ValidatedIdentityGraph,
    types: &dyn mir::MirTypeBridgeTypeLookupV1,
) -> Result<mir::ParamFreeMirCallableBindingV1, Error> {
    let materialization = input
        .module()
        .meta
        .source_callable_materializations
        .get_by_materialization(local.functions[source].materialization)
        .ok_or(Error::MissingMirMaterialization(callable))?;
    let subject = materialization.signature_record().subject();
    let implementation =
        CallableDefinitionOwner::try_from(subject).map_err(|_| Error::InvalidRole(callable))?;
    let roots = input.materialization().callable_roots();
    let signatures = &input.module().meta.callable_signatures;

    let position = roots
        .binary_search_by(|root| root.subject().compare_sort_key(subject))
        .map_err(|_| Error::MissingMirMaterialization(callable))?;
    let actual = signatures
        .get(subject)
        .ok_or(Error::MissingSignature(callable))?;

    let expected = crate::source_callables::exact_function_signature(local, source);

    if actual.signature() != &expected {
        return Err(Error::SignatureMismatch(callable));
    }
    let GeneratedCallableKey::DerivedEquality { exact_owner } = *key else {
        return Err(Error::InvalidRole(callable));
    };
    let gc = crate::lowering_support::lower_gc_effect(local.functions[source].attributes.gc_effect);
    let lowered_gc = input.module().functions[roots[position].function()].gc_effect;

    Ok(mir::ParamFreeMirCallableBindingV1::try_new(
        mir::MirCallableBridgeAuthority {
            identities,
            foundation: input.foundation(),
            types,
        },
        mir::MirCallableOriginV1::Generated {
            callable,
            role: key.clone(),
        },
        implementation,
        mir::MirBridgeCallableSignatureV1::new(expected.clone(), gc),
        mir::MirBridgeCallableSignatureV1::new(expected, lowered_gc),
        mir::MirCallableLoweringRoleV1::DerivedEquality { owner: exact_owner },
    )?)
}
