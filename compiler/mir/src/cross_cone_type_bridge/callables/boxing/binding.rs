use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn project(
    input: &ConeMirInput,
    identities: &ValidatedIdentityGraph,
    types: &dyn MirTypeBridgeTypeLookupV1,
    source: &dyn MirTypeBridgeCallableLookupV1,
    roots: &BTreeMap<FunctionId, CallableSignatureSubject>,
    adjust: &BoxingAdjust,
) -> Result<ParamFreeMirCallableBindingV1, Error> {
    let callable = adjust.identity().callable_record().id();
    let implementation = adjust.identity().signature_record().subject();
    if roots.get(&adjust.function()) != Some(&implementation) {
        return Err(Error::MissingRoot(adjust.function()));
    }
    let (target, target_signature, target_effect) = match adjust.target() {
        crate::BoxingAdjustTarget::Local(function) => {
            let owner = *roots.get(&function).ok_or(Error::MissingRoot(function))?;
            let target = CallableDefinitionOwner::try_from(owner).map_err(Error::InvalidTarget)?;
            let signature = input
                .module()
                .meta
                .callable_signatures
                .get(owner)
                .ok_or(Error::MissingSignature(owner))?
                .signature();
            (
                target,
                signature,
                input.module().functions[function].gc_effect,
            )
        }
        crate::BoxingAdjustTarget::External(callable) => {
            let root = input
                .materialization()
                .external_callable_roots()
                .iter()
                .find(|root| root.callable() == callable)
                .expect("validated external boxing target has a callable root");
            (
                root.implementation().into(),
                root.signature(),
                root.gc_effect(),
            )
        }
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

    let lowered = signatures
        .get(implementation)
        .ok_or(Error::MissingSignature(implementation))?
        .signature();

    if source.semantic_signature() != source.lowered_signature()
        || source.lowered_signature().exact() != target_signature
        || source.lowered_signature().gc_effect() != target_effect
    {
        return Err(Error::TargetMismatch(target));
    }
    if lowered != adjust.identity().signature_record().signature() {
        return Err(Error::SignatureMismatch(implementation));
    }

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
        CallableDefinitionOwner::try_from(implementation).map_err(Error::InvalidTarget)?,
        source.semantic_signature().clone(),
        MirBridgeCallableSignatureV1::new(
            lowered.clone(),
            input.module().functions[adjust.function()].gc_effect,
        ),
        MirCallableLoweringRoleV1::BoxingAdjust { target },
    )?)
}
