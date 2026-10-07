//! Match actual post-call relocations to the pre-RS4GC SSA equivalence groups.

use super::*;

pub(super) fn verify(
    stores: &[(InstructionValue<'_>, u64, ExpectedRoot)],
    observed: &BTreeMap<u64, ObservedStatepoint<'_>>,
    expected: &ExpectedSafepoints,
    space: ManagedAddressSpace,
) -> Result<(), CodegenError> {
    let mut actual = BTreeMap::<u64, ExpectedRoots>::new();
    let mut unique = BTreeSet::new();
    for (store, id, root) in stores {
        let site = observed.get(id).ok_or_else(|| {
            CodegenError(format!(
                "root restoration refers to missing statepoint {id}"
            ))
        })?;
        if !unique.insert((*id, root.key())) {
            return Err(CodegenError(format!(
                "statepoint {id} repeats typed root identity {root:?}"
            )));
        }
        let groups = actual.entry(*id).or_insert_with(|| ExpectedRoots {
            groups: vec![Vec::new(); site.roots.len()],
            constants: Vec::new(),
        });
        let value = root_identity::restored_value(*store, space)?;
        if value.is_const() {
            groups.constants.push(*root);
            continue;
        }
        let relocation = value.as_instruction_value().ok_or_else(|| {
            CodegenError(format!(
                "statepoint {id} restores an unrelocated root {root:?}"
            ))
        })?;
        let index = site
            .relocations
            .iter()
            .find_map(|(index, instruction)| {
                (*instruction == relocation).then_some(*index as usize)
            })
            .ok_or_else(|| {
                CodegenError(format!(
                    "statepoint {id} root {root:?} is not restored from its gc.relocate"
                ))
            })?;
        groups.groups[index].push(*root);
    }
    for (id, site) in &expected.sites {
        let value = actual.remove(id).unwrap_or_default();
        match &site.statepoint {
            ExpectedStatepoint::Relocating(roots) => {
                let constants = |roots: &ExpectedRoots| {
                    roots
                        .constants
                        .iter()
                        .copied()
                        .map(ExpectedRoot::key)
                        .collect::<BTreeSet<_>>()
                };
                if roots.group_keys() != value.group_keys() || constants(roots) != constants(&value)
                {
                    return Err(CodegenError(format!(
                        "statepoint {id} has gc-live identities that disagree with the final GC plan: expected {roots:?}, observed {value:?}"
                    )));
                }
            }
            ExpectedStatepoint::NativeTransition(_) | ExpectedStatepoint::ZeroLiveInvoke => {
                if !value.groups.is_empty() || !value.constants.is_empty() {
                    return Err(CodegenError(format!(
                        "zero-live statepoint {id} has relocation restorations"
                    )));
                }
            }
        }
    }
    Ok(())
}
