use super::*;
use crate::StrongTypeDispatchCallableRefV2;
use scoop_identity::{ConeIdentity, PersistentCallableBodyId};

pub(super) fn replay(
    target: crate::LirTargetProfile,
    input: &ExactDispatchEntryInputV1<'_>,
    foundation: &OdrFreeLirFoundation,
    meter: &mut BudgetMeter,
) -> Result<ExactDispatchEntryV1, ExactDispatchError> {
    let target_owner = input.implementation.target();
    if input.abi.target() != target_owner {
        return Err(ExactDispatchError::AbiTarget(target_owner));
    }
    if input.abi.target_profile() != target {
        return Err(ExactDispatchError::AbiTargetProfile(target_owner));
    }
    if input.abi.physical_definition().provider() == foundation.producer() {
        let expected = StrongShapeDefinitionRefV1::from_foundation(
            ExternalStrongShapeSubjectV1::Callable(target_owner),
            foundation,
            meter,
        )?;
        if input.abi.physical_definition() != expected {
            return Err(ExactDispatchError::AbiDefinition(target_owner));
        }
    }
    let slot_receiver_layout = super::signature::validate(input, target, meter)?;
    let body = input.abi.definition().semantic_id();
    let abi = abi_reference(
        input.abi.physical_definition().provider(),
        foundation.producer(),
        body,
    );
    let count = input.slot_signature.exact().parameters().len() as u64;
    let path = WirePath::root();
    meter.charge_collection_slots(count, &path)?;
    meter.charge_owned_bytes(
        count.saturating_mul(std::mem::size_of::<scoop_identity::PersistentExactTypeId>() as u64),
        &path,
    )?;
    Ok(ExactDispatchEntryV1::from_parts(
        ExactDispatchEntryPartsV1 {
            position: input.position,
            slot: input.slot,
            slot_signature: input.slot_signature.clone(),
            implementation: input.implementation,
            abi,
            callable_abi: input.abi.clone(),
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
