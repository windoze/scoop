use scoop_identity::DefinitionAtomRole;
use scoop_lir::{CallableContextKeyCellV1, StrongCallableRegistrationPlanV1};

use crate::link_object::{
    BuiltinObjectSectionRoleV1 as Section, VerifiedDefinitionAtomRangeV1,
    VerifiedMemberObjectRelocationIndexV1, VerifiedObjectRelocationShapeV1 as Shape,
    VerifiedRelocationTargetV1 as Target,
};

use super::{StrongCallableRegistrationValidationError as Error, atom_file_range};

pub(super) fn verify(
    object: &[u8],
    member: &VerifiedMemberObjectRelocationIndexV1,
    plan: StrongCallableRegistrationPlanV1,
    keys: &[CallableContextKeyCellV1],
) -> Result<(), Error> {
    let invalid = |field| Error::ContextKeys {
        body: plan.body(),
        field,
    };
    let pointers = member
        .relocations()
        .iter()
        .filter(|r| r.containing_atom() == plan.primary_atom())
        .collect::<Vec<_>>();
    if pointers.len() != 1 + usize::from(!keys.is_empty()) {
        return Err(invalid("registration pointer count"));
    }
    if keys.is_empty() {
        return Ok(());
    }
    let body = member
        .definitions()
        .definition(plan.body_definition_plan())
        .ok_or_else(|| invalid("key cells and body must share a LinkObject"))?;
    let table_id = scoop_lir::context_key_table_atom(plan.body_definition_plan())
        .map_err(|_| invalid("table identity"))?;
    let table = body
        .atoms()
        .iter()
        .find(|a| a.atom() == table_id && a.atom_role() == DefinitionAtomRole::ContextKeyTable)
        .copied()
        .ok_or_else(|| invalid("table atom"))?;
    let pointer = pointers
        .iter()
        .find(|r| r.offset_within_atom() == 160)
        .ok_or_else(|| invalid("table pointer"))?;
    if !points_to(pointer.shape(), pointer.encoded_value(), table) {
        return Err(invalid("table pointer target"));
    }
    let (section, start, end) =
        atom_file_range(member, table).map_err(|_| invalid("table range"))?;
    if section != Section::ReadOnlyData || end - start != keys.len() as u64 * 40 {
        return Err(invalid("table layout"));
    }
    let bytes = object
        .get(start as usize..end as usize)
        .ok_or_else(|| invalid("table bytes"))?;
    let pointers = member
        .relocations()
        .iter()
        .filter(|r| r.containing_atom() == table_id)
        .collect::<Vec<_>>();
    if pointers.len() != keys.len() {
        return Err(invalid("table cell pointer count"));
    }
    for (index, key) in keys.iter().enumerate() {
        if &bytes[index * 40..index * 40 + 32] != key.key.0.as_array() {
            return Err(invalid("exact key"));
        }
        let cell = body
            .atoms()
            .iter()
            .find(|a| a.atom() == key.atom && a.atom_role() == DefinitionAtomRole::ContextKeyCell)
            .copied()
            .ok_or_else(|| invalid("cell atom"))?;
        let pointer = pointers
            .iter()
            .find(|r| r.offset_within_atom() == index as u64 * 40 + 32)
            .ok_or_else(|| invalid("cell pointer"))?;
        if !points_to(pointer.shape(), pointer.encoded_value(), cell) {
            return Err(invalid("cell pointer target"));
        }
        let (section, start, end) =
            atom_file_range(member, cell).map_err(|_| invalid("cell range"))?;
        if section != Section::WritableData
            || end - start != 8
            || cell.start() % 8 != 0
            || object.get(start as usize..end as usize) != Some(&[0; 8])
            || member
                .relocations()
                .iter()
                .any(|r| r.containing_atom() == key.atom)
        {
            return Err(invalid("cell layout or initial value"));
        }
    }
    Ok(())
}

fn points_to(shape: &Shape, encoded: u64, range: VerifiedDefinitionAtomRangeV1) -> bool {
    match shape.absolute64_target() {
        Some(Target::LocalDefinition {
            owner_atom: Some(atom),
            section_ordinal,
            value,
            ..
        }) => {
            *atom == range.atom()
                && *section_ordinal == range.section_ordinal()
                && shape.form().absolute64_address(encoded, *value) == Some(range.start())
        }
        Some(Target::SectionBase {
            section_ordinal, ..
        }) => {
            section_ordinal.get() == range.section_ordinal().get()
                && shape.form().absolute64_address(encoded, 0) == Some(range.start())
        }
        _ => false,
    }
}
