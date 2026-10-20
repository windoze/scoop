use std::{collections::BTreeSet, num::NonZeroU32, ops::Range};

use crate::link_object::{
    BuiltinObjectSectionRoleV1, DarwinArm64RelocationShapeV1, DarwinArm64RelocationTargetV1,
    ObjectRelocationShapeV1, ObjectSymbolKindV1, ValidatedBuiltinObjectSectionInventoryV1,
};

pub(super) fn ranges(
    sections: &ValidatedBuiltinObjectSectionInventoryV1,
    ordinal: NonZeroU32,
    atoms: &[Range<u64>],
) -> Vec<Range<u64>> {
    let envelope = sections.envelope();
    let section = &envelope.sections()[ordinal.get() as usize - 1];
    if section.segment_name() != b"__TEXT" || section.section_name() != b"__const" {
        return Vec::new();
    }
    let Some(section_end) = section.virtual_address().checked_add(section.byte_size()) else {
        return Vec::new();
    };
    let referenced = envelope
        .relocations()
        .iter()
        .filter_map(|relocation| {
            if sections.roles()[relocation.containing_section_ordinal().get() as usize - 1]
                != BuiltinObjectSectionRoleV1::Text
            {
                return None;
            }
            match relocation.shape() {
                ObjectRelocationShapeV1::DarwinArm64(
                    DarwinArm64RelocationShapeV1::Page21 {
                        target: DarwinArm64RelocationTargetV1::SymbolTableIndex(index),
                        ..
                    }
                    | DarwinArm64RelocationShapeV1::PageOffset12 {
                        target: DarwinArm64RelocationTargetV1::SymbolTableIndex(index),
                        ..
                    },
                ) => Some(index),
                _ => None,
            }
        })
        .collect::<BTreeSet<_>>();
    let starts = envelope
        .symbols()
        .iter()
        .filter(|symbol| {
            referenced.contains(&symbol.table_index())
                && symbol.kind() == ObjectSymbolKindV1::LocalSectionDefinition
                && symbol.section_ordinal() == Some(ordinal)
                && (section.virtual_address()..section_end).contains(&symbol.value())
                && !atoms.iter().any(|atom| atom.contains(&symbol.value()))
        })
        .map(|symbol| symbol.value())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut ranges = Vec::new();
    for (index, start) in starts.iter().copied().enumerate() {
        let next = starts.get(index + 1).copied().unwrap_or(section_end);
        let end = atoms
            .iter()
            .filter(|atom| atom.start > start)
            .map(|atom| atom.start)
            .min()
            .unwrap_or(next)
            .min(next);
        let range = start..end;
        if envelope.relocations().iter().any(|relocation| {
            relocation.containing_section_ordinal() == ordinal
                && (start - section.virtual_address()..end - section.virtual_address())
                    .contains(&relocation.offset())
        }) {
            continue;
        }
        ranges.push(range);
    }
    ranges
}
