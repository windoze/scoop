use super::*;
use crate::{CallableOwner, CallableSignatureSubject, StrongCallableMaterializationRoot};

pub(super) fn project(
    input: &SingleConeStrongMirInput,
    sources: &[ObjectSource],
    identities: &ValidatedIdentityGraph,
    types: &dyn MirTypeBridgeTypeLookupV1,
    meter: &mut BudgetMeter,
) -> Result<CanonicalMirCallableBindingsV1, MirObjectProductionError> {
    let mut records = Vec::new();
    reserve(&mut records, sources.len().saturating_mul(2), meter)?;
    let authority = MirCallableBridgeAuthority {
        identities,
        foundation: input.foundation(),
        types,
    };
    for source in sources {
        for (root, role, lowering) in [
            (
                source.unit.initializer(),
                InitializationCallableRole::Initializer,
                MirCallableLoweringRoleV1::ObjectInitializer {
                    unit: source.unit.identity(),
                },
            ),
            (
                source.unit.ensure(),
                InitializationCallableRole::Ensure,
                MirCallableLoweringRoleV1::ObjectEnsure {
                    unit: source.unit.identity(),
                },
            ),
        ] {
            let target = implementation(root)?;
            let StrongCallableDefinitionOwner::GeneratedCallable(callable) = target else {
                unreachable!("initialization implementations are generated")
            };
            let signatures = &input.module().meta.callable_signatures;
            meter.charge_work(
                u64::from(signatures.len().checked_ilog2().unwrap_or(0)) + 1,
                &WirePath::root(),
            )?;
            let signature = signatures
                .get(CallableSignatureSubject::Strong(root.implementation()))
                .ok_or(MirObjectProductionError::MissingSignature {
                    implementation: root.implementation(),
                })?
                .signature();
            meter.charge_owned_bytes(
                (signature.parameters().len() as u64)
                    .saturating_mul(2 * std::mem::size_of::<PersistentExactTypeId>() as u64),
                &WirePath::root(),
            )?;
            let signature = MirBridgeCallableSignatureV1::new(
                signature.clone(),
                input.module().functions[root.function()].gc_effect,
            );
            meter.charge_work(
                (signatures.len() as u64).saturating_add(
                    8 * (types.record_count().checked_ilog2().unwrap_or(0) as u64 + 1) + 32,
                ),
                &WirePath::root(),
            )?;
            records.push(ParamFreeMirCallableBindingV1::try_new(
                authority,
                MirCallableOriginV1::Generated {
                    callable,
                    role: GeneratedCallableKey::Initialization {
                        unit: source.unit.identity(),
                        role,
                    },
                },
                target,
                signature.clone(),
                signature,
                lowering,
            )?);
        }
    }
    meter.charge_work(
        (records.len() as u64)
            .saturating_mul(u64::from(records.len().checked_ilog2().unwrap_or(0)) + 1),
        &WirePath::root(),
    )?;
    Ok(CanonicalMirCallableBindingsV1::try_new(records)?)
}

pub(super) fn implementation(
    root: StrongCallableMaterializationRoot,
) -> Result<StrongCallableDefinitionOwner, MirObjectProductionError> {
    match root.implementation() {
        CallableOwner::Generated(callable) => {
            Ok(StrongCallableDefinitionOwner::GeneratedCallable(callable))
        }
        implementation => Err(MirObjectProductionError::InitializationRole { implementation }),
    }
}
