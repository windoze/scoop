//! Bound each referenced numeric table before already-owned object data.

use std::collections::BTreeSet;
use std::num::NonZeroU32;

use crate::link_object::{
    BuiltinObjectSectionRoleV1, DarwinArm64RelocationShapeV1, DarwinArm64RelocationTargetV1,
    ObjectDefinitionV1, ObjectRelocationShapeV1, ObjectSymbolKindV1,
    ValidatedBuiltinObjectSectionInventoryV1,
};

pub(super) fn ranges(
    sections: &ValidatedBuiltinObjectSectionInventoryV1,
    ordinal: NonZeroU32,
    definitions: &[ObjectDefinitionV1],
) -> Vec<(u64, u64)> {
    let envelope = sections.envelope();
    let section = &envelope.sections()[ordinal.get() as usize - 1];
    if section.segment_name() != b"__TEXT"
        || section.section_name() != b"__const"
        || section.byte_size() == 0
    {
        return Vec::new();
    }
    let Some(end) = definitions
        .iter()
        .flat_map(|definition| definition.atoms())
        .filter(|atom| atom.section_ordinal() == ordinal)
        .map(|atom| atom.start())
        .min()
        .or_else(|| section.virtual_address().checked_add(section.byte_size()))
    else {
        return Vec::new();
    };
    let Some(size) = end.checked_sub(section.virtual_address()) else {
        return Vec::new();
    };
    if size == 0
        || envelope.relocations().iter().any(|relocation| {
            relocation.containing_section_ordinal() == ordinal && relocation.offset() < size
        })
    {
        return Vec::new();
    }
    let starts = envelope
        .relocations()
        .iter()
        .filter_map(|relocation| {
            if sections.roles()[relocation.containing_section_ordinal().get() as usize - 1]
                != BuiltinObjectSectionRoleV1::Text
            {
                return None;
            }
            let ObjectRelocationShapeV1::DarwinArm64(
                DarwinArm64RelocationShapeV1::Page21 { target, .. }
                | DarwinArm64RelocationShapeV1::PageOffset12 { target, .. },
            ) = relocation.shape()
            else {
                return None;
            };
            let DarwinArm64RelocationTargetV1::SymbolTableIndex(index) = target else {
                return None;
            };
            envelope.symbols().iter().find_map(|symbol| {
                (symbol.table_index() == index
                    && symbol.kind() == ObjectSymbolKindV1::LocalSectionDefinition
                    && symbol.section_ordinal() == Some(ordinal)
                    && symbol.value() >= section.virtual_address()
                    && symbol.value() < end)
                    .then_some(symbol.value())
            })
        })
        .collect::<BTreeSet<_>>();
    let mut boundaries = starts.into_iter().collect::<Vec<_>>();
    boundaries.push(end);
    boundaries
        .windows(2)
        .map(|pair| (pair[0] - section.virtual_address(), pair[1] - pair[0]))
        .collect()
}
