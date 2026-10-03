use super::*;
use crate::link::map::LinkMap;
use scoop_identity::{ConeIdentity, ObjectDefinitionPlanOwner};
use scoop_slib::SlibMemberId;

type ObjectKey = (ConeIdentity, SlibMemberId);
pub(super) type Winners = BTreeMap<ObjectDefinitionAtomId, ObjectKey>;

#[cfg(test)]
mod tests;

pub(super) fn winners(
    image: &FinalImage<'_>,
    inputs: &ProgramInputs<'_>,
    map: &LinkMap,
) -> Result<Winners, LinkError> {
    let mut candidates: BTreeMap<_, (&[u8], BTreeSet<ObjectKey>)> = BTreeMap::new();
    for closure in &inputs.strong_relocations {
        for member in closure.members() {
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
                if let SymbolRole::AtomBoundaryStart { atom, .. } = symbol.role()
                    && matches!(
                        symbol.definition_owner(),
                        ObjectDefinitionPlanOwner::Odr { .. }
                    )
                    && checked.contains(&atom)
                {
                    candidates
                        .entry(atom)
                        .or_insert_with(|| (symbol.macho_name(), BTreeSet::new()))
                        .1
                        .insert((member.producer(), member.member()));
                }
            }
        }
    }
    candidates
        .into_iter()
        .filter(|(_, (_, objects))| objects.len() > 1)
        .map(|(atom, (name, objects))| {
            let name = std::str::from_utf8(name).map_err(error)?;
            retained(map, name, &objects, image.symbol(name)?).map(|object| (atom, object))
        })
        .collect()
}

fn retained(
    map: &LinkMap,
    name: &str,
    candidates: &BTreeSet<ObjectKey>,
    address: u64,
) -> Result<ObjectKey, LinkError> {
    let [row] = map.cones.get(name).map(Vec::as_slice).unwrap_or(&[]) else {
        return Err(error(format!(
            "ODR atom {name} has no unique retained object in link map"
        )));
    };
    let object = (row.cone, row.member);
    if !candidates.contains(&object) {
        return Err(error(format!(
            "ODR atom {name} resolves to a link map object outside its candidates"
        )));
    }
    if row.address != address {
        return Err(error(format!(
            "ODR atom {name} link map address differs from final symbol table"
        )));
    }
    Ok(object)
}
