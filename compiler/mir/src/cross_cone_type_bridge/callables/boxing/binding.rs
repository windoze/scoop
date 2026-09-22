use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn project(
    input: &SingleConeStrongMirInput,
    identities: &ValidatedIdentityGraph,
    types: &dyn MirTypeBridgeTypeLookupV1,
    source: &dyn MirTypeBridgeCallableLookupV1,
    roots: &BTreeMap<FunctionId, CallableOwner>,
    adjust: &BoxingAdjust,
    meter: &mut BudgetMeter,
) -> Result<ParamFreeMirCallableBindingV1, Error> {
    work(
        2 * search_cost(roots.len()) + search_cost(source.record_count()),
        meter,
    )?;
    let callable = adjust.identity().callable_record().id();
    let implementation = CallableOwner::Generated(callable);
    if roots.get(&adjust.function()) != Some(&implementation) {
        return Err(Error::MissingStrongRoot(adjust.function()));
    }
    let target_owner = *roots
        .get(&adjust.target())
        .ok_or(Error::MissingStrongRoot(adjust.target()))?;
    let target = match target_owner {
        CallableOwner::Function(id) => StrongCallableDefinitionOwner::Function(id),
        CallableOwner::Accessor(id) => StrongCallableDefinitionOwner::PropertyAccessor(id),
        _ => return Err(Error::InvalidTarget(target_owner)),
    };
    let source = source
        .get(target)
        .ok_or(Error::MissingTargetBinding(target))?;
    if !matches!(
        source.lowering_role(),
        MirCallableLoweringRoleV1::Ordinary | MirCallableLoweringRoleV1::Accessor
    ) {
        return Err(Error::TargetMismatch(target));
    }
    let signatures = &input.module().meta.callable_signatures;
    work(2 * search_cost(signatures.len()), meter)?;
    let target_signature = signatures
        .get(CallableSignatureSubject::Strong(target_owner))
        .ok_or(Error::MissingSignature(target_owner))?
        .signature();
    let lowered = signatures
        .get(CallableSignatureSubject::Strong(implementation))
        .ok_or(Error::MissingSignature(implementation))?
        .signature();
    let parameters = target_signature.parameters().len() + lowered.parameters().len();
    work((parameters as u64 + 1) * 2, meter)?;
    if source.semantic_signature() != source.lowered_signature()
        || source.lowered_signature().exact() != target_signature
        || source.lowered_signature().gc_effect()
            != input.module().functions[adjust.target()].gc_effect
    {
        return Err(Error::TargetMismatch(target));
    }
    if lowered != adjust.identity().signature_record().signature() {
        return Err(Error::SignatureMismatch(implementation));
    }
    meter.charge_collection_slots(parameters as u64, &WirePath::root())?;
    meter.charge_owned_bytes(
        (parameters as u64).saturating_mul(std::mem::size_of::<PersistentExactTypeId>() as u64),
        &WirePath::root(),
    )?;
    work(
        (signatures.len() as u64).saturating_mul(2).saturating_add(
            (parameters as u64 * 8 + 16).saturating_mul(search_cost(types.record_count())) + 32,
        ),
        meter,
    )?;
    Ok(ParamFreeMirCallableBindingV1::try_new(
        MirCallableBridgeAuthority {
            identities,
            foundation: input.foundation(),
            types,
        },
        MirCallableOriginV1::Generated {
            callable,
            role: adjust.identity().callable_record().key().clone(),
        },
        StrongCallableDefinitionOwner::GeneratedCallable(callable),
        source.semantic_signature().clone(),
        MirBridgeCallableSignatureV1::new(
            lowered.clone(),
            input.module().functions[adjust.function()].gc_effect,
        ),
        MirCallableLoweringRoleV1::BoxingAdjust { target },
    )?)
}
