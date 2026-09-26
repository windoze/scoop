use super::*;
use crate::StrongTypeDispatchCallableRefV2;
use scoop_identity::{ConeIdentity, PersistentCallableBodyId};

pub(super) fn replay(
    target: crate::LirTargetProfile,
    input: &ExactDispatchEntryInputV1<'_>,
    foundation: &OdrFreeLirFoundation,
) -> Result<ExactDispatchEntryV1, ExactDispatchError> {
    let target_owner = input.implementation.target();
    if input.abi.target() != target_owner {
        return Err(ExactDispatchError::AbiTarget(target_owner));
    }
    if input.abi.target_profile() != target {
        return Err(ExactDispatchError::AbiTargetProfile(target_owner));
    }
    if input.abi.calling_convention() != crate::CallingConvention::Cdecl {
        return Err(ExactDispatchError::AbiSignature(target_owner));
    }
    if input.abi.provider() == foundation.producer() {
        let expected = StrongShapeDefinitionRefV1::from_foundation(
            ExternalStrongShapeSubjectV1::Callable(target_owner),
            foundation,
        )?;
        if !input.abi.matches_definition(expected) {
            return Err(ExactDispatchError::AbiDefinition(target_owner));
        }
    }
    let slot_receiver_layout = super::signature::validate(input, target)?;
    let body = input.abi.body()?;
    let abi = abi_reference(input.abi.provider(), foundation.producer(), body);

    Ok(ExactDispatchEntryV1::from_parts(
        ExactDispatchEntryPartsV1 {
            position: input.position,
            slot: input.slot,
            slot_signature: input.slot_signature.clone(),
            implementation: input.implementation,
            abi,
            slot_receiver_layout,
        },
    ))
}

fn abi_reference(
    provider: ConeIdentity,
    current: ConeIdentity,
    body: PersistentCallableBodyId,
) -> StrongTypeDispatchCallableRefV2 {
    if provider == current {
        StrongTypeDispatchCallableRefV2::Local(body)
    } else {
        StrongTypeDispatchCallableRefV2::DependencyExternal { provider, body }
    }
}
