//! Actual startup helpers share the ordinary generated callable table.

use super::*;

pub(super) fn append(
    input: &mir::ConeMirInput,
    identities: &ValidatedIdentityGraph,
    types: &dyn mir::MirTypeBridgeTypeLookupV1,
    records: &mut Vec<mir::ParamFreeMirCallableBindingV1>,
) -> Result<(), Error> {
    for start in &input.module().meta.coroutine_starts {
        let identity = start.identity();
        let callable = identity.callable_record();
        let signature = mir::MirBridgeCallableSignatureV1::new(
            identity.signature_record().signature().clone(),
            mir::GcEffect::Managed,
        );
        let implementation = scoop_identity::CallableDefinitionOwner::try_from(
            identity.signature_record().subject(),
        )
        .expect("a coroutine start has a Strong or ODR callable definition");
        records.push(mir::ParamFreeMirCallableBindingV1::try_new(
            mir::MirCallableBridgeAuthority {
                identities,
                foundation: input.foundation(),
                types,
            },
            mir::MirCallableOriginV1::Generated {
                callable: callable.id(),
                role: callable.key().clone(),
            },
            implementation,
            signature.clone(),
            signature,
            mir::MirCallableLoweringRoleV1::CoroutineStart,
        )?);
    }
    Ok(())
}
