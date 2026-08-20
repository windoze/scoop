//! Definition-owned storage bridges for parameter-free native callback addresses.

use super::*;

pub(super) fn append(
    input: &mir::ConeMirInput,
    identities: &ValidatedIdentityGraph,
    types: &dyn mir::MirTypeBridgeTypeLookupV1,
    ordinary: &[mir::ParamFreeMirCallableExportV1],
    records: &mut Vec<mir::ParamFreeMirCallableBindingV1>,
) -> Result<(), Error> {
    for (_, callback) in input.module().callback_bridges.iter() {
        if callback.local_definition().is_none()
            || callback.identity().source().context()
                != CallableMaterializationContext::NoSubstitution
        {
            continue;
        }
        let identity = callback.identity();
        let CallableTemplateOwner::Function(source) = identity.source().template() else {
            continue;
        };
        let source = scoop_identity::StrongCallableDefinitionOwner::Function(source);
        if !ordinary
            .iter()
            .any(|callable| callable.implementation() == source)
            && !records
                .iter()
                .any(|callable| callable.implementation() == source.into())
        {
            continue;
        }
        let key = identity.callable_record();
        let scoop_identity::GeneratedCallableKey::StaticNoGcCallbackStorageBridge {
            signature, ..
        } = key.key()
        else {
            unreachable!("a static callback owns its storage bridge key")
        };
        records.push(mir::ParamFreeMirCallableBindingV1::try_new(
            mir::MirCallableBridgeAuthority {
                identities,
                foundation: input.foundation(),
                types,
            },
            mir::MirCallableOriginV1::Generated {
                callable: key.id(),
                role: key.key().clone(),
            },
            scoop_identity::StrongCallableDefinitionOwner::GeneratedCallable(key.id()),
            mir::MirBridgeCallableSignatureV1::new(signature.clone(), mir::GcEffect::NoGc),
            mir::MirBridgeCallableSignatureV1::new(
                identity.signature_record().signature().clone(),
                mir::GcEffect::NoGc,
            ),
            mir::MirCallableLoweringRoleV1::StaticCallbackStorage,
        )?);
    }
    Ok(())
}
