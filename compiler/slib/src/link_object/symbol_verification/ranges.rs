//! Only complete immutable dispatch views may share native atom ranges.

use std::{cmp::Reverse, collections::BTreeSet};

use super::*;
use crate::link_object::BuiltinObjectSectionRoleV1;

pub(super) fn validate(
    definitions: &[ObjectDefinitionV1],
    symbols: &[ObjectDefinitionSymbolV1],
    sections: &ValidatedBuiltinObjectSectionInventoryV1,
) -> Result<(), ObjectDefinitionReadError> {
    let dispatch = symbols
        .iter()
        .filter_map(|symbol| match symbol.role() {
            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                primary_atom,
                definition_role: scoop_identity::StrongDefinitionRole::DispatchTable,
                ..
            } => Some(primary_atom),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut ranges = definitions
        .iter()
        .flat_map(|definition| definition.atoms.iter().copied())
        .collect::<Vec<_>>();
    ranges.sort_unstable_by_key(|range| {
        (
            range.section_ordinal,
            range.start,
            Reverse(range.end),
            range.atom,
        )
    });
    let mut active = Vec::<DefinitionAtomRangeV1>::new();
    for range in ranges {
        while active.last().is_some_and(|outer| {
            outer.section_ordinal != range.section_ordinal || outer.end <= range.start
        }) {
            active.pop();
        }
        if let Some(outer) = active.last() {
            let read_only = sections.roles()[range.section_ordinal.get() as usize - 1]
                == BuiltinObjectSectionRoleV1::ReadOnlyData;
            if range.end > outer.end
                || !read_only
                || !dispatch.contains(&range.atom)
                || !dispatch.contains(&outer.atom)
            {
                return Err(ObjectDefinitionReadError::OverlappingAtomRanges {
                    first: outer.atom,
                    second: range.atom,
                });
            }
        }
        active.push(range);
    }
    Ok(())
}
