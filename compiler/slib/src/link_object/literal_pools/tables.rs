use std::{collections::BTreeSet, num::NonZeroU32, ops::Range};

use crate::link_object::{
    BuiltinObjectSectionRoleV1, MachORelocationShapeV1, ObjectRelocationShapeV1,
    ObjectRelocationTargetV1, ObjectSymbolKindV1, ValidatedBuiltinObjectSectionInventoryV1,
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
    let starts = envelope
        .relocations()
        .iter()
        .filter_map(|relocation| {
            if sections.roles()[relocation.containing_section_ordinal().get() as usize - 1]
                != BuiltinObjectSectionRoleV1::Text
            {
                return None;
            }
            let (symbol_index, addend) = match relocation.shape() {
                ObjectRelocationShapeV1::MachO(
                    MachORelocationShapeV1::Page21 {
                        target: ObjectRelocationTargetV1::SymbolTableIndex(index),
                        ..
                    }
                    | MachORelocationShapeV1::PageOffset12 {
                        target: ObjectRelocationTargetV1::SymbolTableIndex(index),
                        ..
                    },
                ) => (index, 0),
                ObjectRelocationShapeV1::MachO(MachORelocationShapeV1::X86_64PcRelative32 {
                    kind,
                    target,
                }) => match target {
                    ObjectRelocationTargetV1::SectionOrdinal(target) if target == ordinal => {
                        let source = &envelope.sections()
                            [relocation.containing_section_ordinal().get() as usize - 1];
                        return kind.section_target_address(
                            relocation.encoded_value(),
                            source.virtual_address().checked_add(relocation.offset())?,
                        );
                    }
                    ObjectRelocationTargetV1::SymbolTableIndex(index) => (
                        index,
                        i128::from(relocation.encoded_value() as i32)
                            + i128::from(kind.trailing_bytes()),
                    ),
                    _ => return None,
                },
                _ => return None,
            };
            let symbol = envelope.symbols().get(symbol_index as usize)?;
            (symbol.kind() == ObjectSymbolKindV1::LocalSectionDefinition
                && symbol.section_ordinal() == Some(ordinal))
            .then(|| u64::try_from(i128::from(symbol.value()) + addend).ok())
            .flatten()
        })
        .filter(|address| {
            (section.virtual_address()..section_end).contains(address)
                && !atoms.iter().any(|atom| atom.contains(address))
        })
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
