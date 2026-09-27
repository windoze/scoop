use super::*;
use crate::{CallableMaterializationRoot, CallableOwner, CallableSignatureSubject};

pub(super) fn project(
    input: &ConeMirInput,
    sources: &[ObjectSource],
    identities: &ValidatedIdentityGraph,
    types: &dyn MirTypeBridgeTypeLookupV1,
) -> Result<CanonicalMirCallableBindingsV1, MirObjectProductionError> {
    let mut records = Vec::new();
    reserve(&mut records, sources.len().saturating_mul(2))?;
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

            let signature = signatures
                .get(root.subject())
                .ok_or(MirObjectProductionError::MissingSignature {
                    implementation: target.callable_owner(),
                })?
                .signature();

            let signature = MirBridgeCallableSignatureV1::new(
                signature.clone(),
                input.module().functions[root.function()].gc_effect,
            );

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

    Ok(CanonicalMirCallableBindingsV1::try_new(records)?)
}

pub(super) fn implementation(
    root: CallableMaterializationRoot,
) -> Result<StrongCallableDefinitionOwner, MirObjectProductionError> {
    match root.subject() {
        CallableSignatureSubject::Strong(CallableOwner::Generated(callable)) => {
            Ok(StrongCallableDefinitionOwner::GeneratedCallable(callable))
        }
        implementation => Err(MirObjectProductionError::InitializationRole { implementation }),
    }
}
