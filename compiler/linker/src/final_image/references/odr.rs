use super::*;
use crate::link::map::LinkMap;
use scoop_identity::{ConeIdentity, ObjectDefinitionPlanOwner};
use scoop_slib::SlibMemberId;

type ObjectKey = (ConeIdentity, SlibMemberId);

#[cfg(test)]
mod tests;

pub(super) fn check(
    image: &FinalImage<'_>,
    inputs: &ProgramInputs<'_>,
    map: &LinkMap,
) -> Result<(), LinkError> {
    for closure in &inputs.strong_relocations {
        for member in closure.members() {
            if !inputs.selected.contains(member.producer(), member.member()) {
                continue;
            }
            let checked: BTreeSet<_> = member
                .relocations()
                .iter()
                .filter(|use_| {
                    !matches!(
                        use_.containing_atom_role(),
                        DefinitionAtomRole::EhFrame
                            | DefinitionAtomRole::CompactUnwind
                            | DefinitionAtomRole::Stackmap
                    ) && controlled(use_.shape())
                })
                .map(|use_| use_.containing_atom())
                .collect();
            for symbol in member.definitions().symbols() {
                let retained_symbol = match symbol.role() {
                    SymbolRole::PrimaryDefinition { .. } => true,
                    SymbolRole::AtomBoundaryStart { atom, .. } => checked.contains(&atom),
                    _ => false,
                };
                if retained_symbol
                    && matches!(
                        symbol.definition_owner(),
                        ObjectDefinitionPlanOwner::Odr { .. }
                    )
                {
                    let name = std::str::from_utf8(symbol.macho_name()).map_err(error)?;
                    retained(
                        map,
                        name,
                        (member.producer(), member.member()),
                        image.symbol(name)?,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn retained(map: &LinkMap, name: &str, selected: ObjectKey, address: u64) -> Result<(), LinkError> {
    let [row] = map.cones.get(name).map(Vec::as_slice).unwrap_or(&[]) else {
        return Err(error(format!(
            "ODR atom {name} has no unique retained object in link map"
        )));
    };
    let object = (row.cone, row.member);
    if object != selected {
        return Err(error(format!(
            "ODR atom {name} resolves to a link map object different from its selected implementation"
        )));
    }
    if row.address != address {
        return Err(error(format!(
            "ODR atom {name} link map address differs from final symbol table"
        )));
    }
    Ok(())
}
