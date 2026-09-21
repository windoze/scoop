use scoop_identity::{
    ConeIdentity, DispatchTableRole, OptionalExactInterface, PersistentCallableBodyId,
    RepresentationRole,
};
use scoop_wire::{BudgetMeter, WirePath};

use super::*;
use crate::{
    ExactRepresentationKindV1, ExternalStrongShapeSubjectV1, NichePointerKind,
    OdrFreeLirFoundation, StrongShapeDefinitionRefV1, StrongShapeDefinitionV1,
    StrongTypeDispatchCallableRefV2,
};

impl ExactDispatchExportV1 {
    pub fn replay(
        target: crate::LirTargetProfile,
        physical: ExactDispatchPhysicalTableV1<'_>,
        inputs: &[ExactDispatchEntryInputV1<'_>],
        foundation: &OdrFreeLirFoundation,
        resolver: &mut impl ExactDispatchPhysicalCallableResolverV1,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ExactDispatchError> {
        let path = WirePath::root();
        let count = inputs.len() as u64;
        meter.check_table_entries(count, &path)?;
        let retained_slots = count
            .checked_mul(2)
            .ok_or(ExactDispatchError::CountOverflow)?;
        meter.charge_collection_slots(retained_slots, &path)?;
        if physical.slots().len() != inputs.len() {
            return Err(ExactDispatchError::SlotCount {
                expected: physical.slots().len(),
                actual: inputs.len(),
            });
        }

        let identity = physical.identity();
        meter.charge_work(foundation.dispatch_tables().len() as u64, &path)?;
        let Some(canonical) = foundation
            .dispatch_tables()
            .iter()
            .find(|record| record.id() == identity.id())
        else {
            return Err(ExactDispatchError::MissingFoundationTable(identity.id()));
        };
        if canonical.key() != identity.key() {
            return Err(ExactDispatchError::FoundationTableKey(identity.id()));
        }
        let (owner_exact, role) = checked_role(identity.key(), identity.id())?;

        let comparison_work = count
            .checked_mul(u64::from(count.max(1).ilog2()) + 1)
            .ok_or(ExactDispatchError::CountOverflow)?;
        meter.charge_work(comparison_work, &path)?;
        let mut seen_slots = Vec::new();
        meter.try_reserve_collection_slots(&mut seen_slots, inputs.len(), &path)?;
        for (index, input) in inputs.iter().enumerate() {
            let expected_position =
                u32::try_from(index).map_err(|_| ExactDispatchError::CountOverflow)?;
            if input.position.into_u32() != expected_position {
                return Err(ExactDispatchError::Position {
                    expected: expected_position,
                    actual: input.position.into_u32(),
                });
            }
            seen_slots.push(input.slot);
        }
        seen_slots.sort_unstable();
        if let Some(pair) = seen_slots.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(ExactDispatchError::DuplicateSlot(pair[0]));
        }
        let mut entries = Vec::new();
        meter.try_reserve_collection_slots(&mut entries, inputs.len(), &path)?;
        for (input, emitted) in inputs.iter().zip(physical.slots()) {
            entries.push(replay_entry(
                target,
                input,
                emitted.callable,
                foundation,
                resolver,
                meter,
            )?);
        }

        let physical_definition = StrongShapeDefinitionRefV1::from_foundation(
            ExternalStrongShapeSubjectV1::DispatchTable(identity.id()),
            foundation,
            meter,
        )?;
        let definition = StrongShapeDefinitionV1::from_artifact(
            identity.id(),
            physical_definition.definition(),
            physical_definition.symbol(),
        );
        Ok(Self::from_parts(ExactDispatchBodyPartsV1 {
            table: identity.id(),
            owner_exact,
            target,
            role,
            entries,
            physical: physical_definition,
            definition,
        }))
    }
}

fn checked_role(
    key: &scoop_identity::DispatchTableKey,
    table: scoop_identity::PersistentDispatchTableId,
) -> Result<(scoop_identity::PersistentExactTypeId, ExactDispatchRoleV1), ExactDispatchError> {
    let role = match (key.role(), key.interface()) {
        (DispatchTableRole::VTable, OptionalExactInterface::Absent) => ExactDispatchRoleV1::Vtable,
        (DispatchTableRole::ITable, OptionalExactInterface::Present(interface_exact)) => {
            ExactDispatchRoleV1::Itable { interface_exact }
        }
        _ => return Err(ExactDispatchError::FoundationTableKey(table)),
    };
    Ok((key.exact_type(), role))
}

fn replay_entry(
    target: crate::LirTargetProfile,
    input: &ExactDispatchEntryInputV1<'_>,
    emitted: crate::CallableRef,
    foundation: &OdrFreeLirFoundation,
    resolver: &mut impl ExactDispatchPhysicalCallableResolverV1,
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
    let slot_receiver_layout = validate_signature(input, target)?;
    let body = input.abi.definition().semantic_id();
    let abi = abi_reference(
        input.abi.physical_definition().provider(),
        foundation.producer(),
        body,
    );
    meter.charge_work(1, &WirePath::root())?;
    let Some(actual) = resolver.resolve(emitted, meter)? else {
        return Err(ExactDispatchError::MissingPhysicalCallable(
            input.position.into_u32(),
        ));
    };
    if actual != abi {
        return Err(ExactDispatchError::PhysicalCallable(
            input.position.into_u32(),
        ));
    }
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

fn validate_signature(
    input: &ExactDispatchEntryInputV1<'_>,
    target: crate::LirTargetProfile,
) -> Result<Option<std::sync::Arc<crate::ExactValueLayoutV1>>, ExactDispatchError> {
    let owner = input.implementation.target();
    let target_signature = input.abi.canonical_signature();
    if input.slot_signature.gc_effect() != target_signature.gc_effect() {
        return Err(ExactDispatchError::AbiSignature(owner));
    }
    let slot = input.slot_signature.exact();
    let implementation = target_signature.signature();
    match input.implementation.receiver_adaptation() {
        ExactDispatchReceiverAdaptationV1::Identity => {
            if slot != implementation {
                return Err(ExactDispatchError::AbiSignature(owner));
            }
            if input.slot_receiver_layout.is_some() {
                return Err(ExactDispatchError::UnexpectedReceiverLayout(owner));
            }
            Ok(None)
        }
        ExactDispatchReceiverAdaptationV1::ReferenceDispatch => {
            if slot.effect() != implementation.effect()
                || slot.parameters() != implementation.parameters()
                || slot.result() != implementation.result()
            {
                return Err(ExactDispatchError::AbiSignature(owner));
            }
            let (Some(slot_receiver), Some(implementation_receiver)) = (
                slot.receiver().into_option(),
                implementation.receiver().into_option(),
            ) else {
                return Err(ExactDispatchError::ReceiverAdaptation(owner));
            };
            if slot_receiver == implementation_receiver {
                return Err(ExactDispatchError::ReceiverAdaptation(owner));
            }
            let Some(slot_layout) = input.slot_receiver_layout else {
                return Err(ExactDispatchError::ReceiverLayout(owner));
            };
            if slot_layout.identity().target() != target
                || slot_layout.identity().exact() != slot_receiver
                || slot_layout.identity().layout_key().representation()
                    != RepresentationRole::ManagedValue
            {
                return Err(ExactDispatchError::ReceiverLayout(owner));
            }
            let Some(slot_layout) = slot_layout.value_handle() else {
                return Err(ExactDispatchError::ReceiverLayout(owner));
            };
            let Some(target_layout) = input.abi.layout_dependencies().receiver().value() else {
                return Err(ExactDispatchError::ReceiverLayout(owner));
            };
            if !managed_reference(&slot_layout) || !managed_reference(target_layout) {
                return Err(ExactDispatchError::ReceiverLayout(owner));
            }
            Ok(Some(slot_layout))
        }
    }
}

fn managed_reference(value: &crate::ExactValueLayoutV1) -> bool {
    matches!(
        value.representation().kind(),
        ExactRepresentationKindV1::QualifiedPointer(NichePointerKind::Managed)
    )
}
