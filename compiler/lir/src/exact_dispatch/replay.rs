use scoop_identity::{
    CborIdentityRecord, DispatchTableKey, DispatchTableRole, OptionalExactInterface,
    PersistentDispatchTableId,
};
use scoop_wire::WirePath;

use super::*;
use crate::{
    ConeLirFoundation, ExternalStrongShapeSubjectV1, StrongShapeDefinitionRefV1,
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
        foundation: &ConeLirFoundation,
    ) -> Result<Self, ExactDispatchError> {
        let path = WirePath::root();

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

        let mut seen_slots = Vec::new();
        scoop_wire::allocation::try_reserve(&mut seen_slots, inputs.len(), &path)?;
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
        scoop_wire::allocation::try_reserve(&mut entries, inputs.len(), &path)?;
        for input in inputs {
            entries.push(entry::replay(target, input, foundation)?);
        }

        let physical_definition = StrongShapeDefinitionRefV1::from_foundation(
            ExternalStrongShapeSubjectV1::DispatchTable(identity.id()),
            foundation,
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
