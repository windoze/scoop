use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn project(
    local: &hir::concrete::Module,
    source: hir::concrete::FunctionId,
    input: &mir::SingleConeStrongMirInput,
    callable: PersistentGeneratedCallableId,
    key: &GeneratedCallableKey,
    identities: &ValidatedIdentityGraph,
    types: &dyn mir::MirTypeBridgeTypeLookupV1,
    meter: &mut BudgetMeter,
) -> Result<mir::ParamFreeMirCallableBindingV1, Error> {
    let implementation = CallableOwner::Generated(callable);
    let roots = input.materialization().callable_roots();
    let signatures = &input.module().meta.callable_signatures;
    meter.charge_work(
        search(roots.len()) + search(signatures.len()),
        &WirePath::root(),
    )?;
    let position = roots
        .binary_search_by_key(&implementation, |root| root.implementation())
        .map_err(|_| Error::MissingMirMaterialization(callable))?;
    let actual = signatures
        .get(mir::CallableSignatureSubject::Strong(implementation))
        .ok_or(Error::MissingSignature(callable))?;
    let parameter_count = local.functions[source].params.len();
    meter.charge_collection_slots(parameter_count as u64 * 2, &WirePath::root())?;
    meter.charge_owned_bytes(
        parameter_count as u64
            * 2
            * std::mem::size_of::<scoop_identity::PersistentExactTypeId>() as u64,
        &WirePath::root(),
    )?;
    let expected = crate::source_callables::exact_function_signature(local, source);
    meter.charge_work(parameter_count as u64 + 1, &WirePath::root())?;
    if actual.signature() != &expected {
        return Err(Error::SignatureMismatch(callable));
    }
    let GeneratedCallableKey::DerivedEquality { exact_owner } = *key else {
        return Err(Error::InvalidRole(callable));
    };
    let gc = crate::lowering_support::lower_gc_effect(local.functions[source].attributes.gc_effect);
    let lowered_gc = input.module().functions[roots[position].function()].gc_effect;
    meter.charge_work(
        signatures.len() as u64 * 2
            + (parameter_count as u64 * 8 + 16) * search(types.record_count())
            + 32,
        &WirePath::root(),
    )?;
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
        StrongCallableDefinitionOwner::GeneratedCallable(callable),
        mir::MirBridgeCallableSignatureV1::new(expected.clone(), gc),
        mir::MirBridgeCallableSignatureV1::new(expected, lowered_gc),
        mir::MirCallableLoweringRoleV1::DerivedEquality { owner: exact_owner },
    )?)
}
