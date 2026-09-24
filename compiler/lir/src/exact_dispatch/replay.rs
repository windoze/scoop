use scoop_identity::{
    CborIdentityRecord, DispatchTableKey, DispatchTableRole, OptionalExactInterface,
    PersistentDispatchTableId,
};
use scoop_wire::{BudgetMeter, WirePath};

use super::*;
use crate::{
    ExternalStrongShapeSubjectV1, OdrFreeLirFoundation, StrongShapeDefinitionRefV1,
    StrongShapeDefinitionV1,
};

mod entry;
mod signature;

impl ExactDispatchExportV1 {
    /// Replays canonical entries without claiming an emitted physical table.
    pub fn replay_from_schema(
        target: crate::LirTargetProfile,
        identity: &CborIdentityRecord<PersistentDispatchTableId, DispatchTableKey>,
        inputs: &[ExactDispatchEntryInputV1<'_>],
        foundation: &OdrFreeLirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ExactDispatchError> {
        let path = WirePath::root();
        let count = inputs.len() as u64;
        meter.check_table_entries(count, &path)?;
        let retained_slots = count
            .checked_mul(2)
            .ok_or(ExactDispatchError::CountOverflow)?;
        meter.charge_collection_slots(retained_slots, &path)?;
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
        for input in inputs {
            entries.push(entry::replay(target, input, foundation, meter)?);
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
