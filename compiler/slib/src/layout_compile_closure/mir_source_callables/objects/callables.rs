use super::*;
use scoop_identity::{
    CoreBuiltinNominal, Effect, GeneratedCallableKey, InitializationCallableRole,
    PersistentGeneratedCallableId, StrongCallableDefinitionOwner,
};

pub(super) fn validate(
    metadata: hir::SharedTypeMetadataV1<'_>,
    callables: &mir::CanonicalMirCallableBindingsV1,
    unit: PersistentInitializationUnitId,
) -> Result<StrongCallableDefinitionOwner, Error> {
    let result = metadata.signature_exact_type(&SignatureTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))?;
    let initializer = identity(unit, InitializationCallableRole::Initializer)?;
    let ensure = identity(unit, InitializationCallableRole::Ensure)?;
    for (role, callable) in [
        (InitializationCallableRole::Initializer, initializer),
        (InitializationCallableRole::Ensure, ensure),
    ] {
        let target = StrongCallableDefinitionOwner::GeneratedCallable(callable);

        let binding = callables
            .get(target)
            .ok_or(Error::MissingCallable(callable))?;
        let expected_role = match role {
            InitializationCallableRole::Initializer => {
                mir::MirCallableLoweringRoleV1::ObjectInitializer { unit }
            }
            InitializationCallableRole::Ensure => {
                mir::MirCallableLoweringRoleV1::ObjectEnsure { unit }
            }
        };
        Error::callable(
            callable,
            Component::CallableOrigin,
            binding.origin()
                == &mir::MirCallableOriginV1::Generated {
                    callable,
                    role: GeneratedCallableKey::Initialization { unit, role },
                },
        )?;
        Error::callable(
            callable,
            Component::CallableRole,
            *binding.lowering_role() == expected_role,
        )?;
        for signature in [binding.semantic_signature(), binding.lowered_signature()] {
            let exact = signature.exact();

            Error::callable(
                callable,
                Component::Signature,
                signature.gc_effect() == mir::GcEffect::Managed
                    && exact.effect() == Effect::Ordinary
                    && !exact.receiver().is_present()
                    && exact.parameters().is_empty()
                    && exact.result() == result,
            )?;
        }
    }
    Ok(StrongCallableDefinitionOwner::GeneratedCallable(ensure))
}

fn identity(
    unit: PersistentInitializationUnitId,
    role: InitializationCallableRole,
) -> Result<PersistentGeneratedCallableId, Error> {
    let key = GeneratedCallableKey::Initialization { unit, role };

    PersistentGeneratedCallableId::from_key(&key).map_err(Error::Key)
}
