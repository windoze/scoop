use super::*;

type ExpectedRuntimeScanAtom = (Vec<u8>, Vec<(u64, scoop_identity::ObjectDefinitionAtomId)>);

pub(super) fn exact_runtime_scan_ranges(
    definition: &crate::link_object::VerifiedStrongObjectDefinitionV1,
    plan: &StrongCallableRuntimeScanPlanV1,
    body: PersistentCallableBodyId,
) -> Result<
    Vec<crate::link_object::VerifiedDefinitionAtomRangeV1>,
    StrongCallableBodyFingerprintError,
> {
    let mut expected = plan
        .atoms()
        .iter()
        .map(|atom| atom.atom())
        .collect::<Vec<_>>();
    expected.sort_unstable();
    let mut actual = definition
        .atoms()
        .iter()
        .filter(|atom| atom.atom_role() == DefinitionAtomRole::RuntimeRecord)
        .map(|atom| atom.atom())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    if actual != expected {
        return Err(
            StrongCallableBodyFingerprintError::RuntimeScanAtomSetMismatch {
                body,
                expected,
                actual,
            },
        );
    }
    plan.atoms()
        .iter()
        .map(|planned| {
            definition
                .atoms()
                .iter()
                .find(|atom| {
                    atom.atom() == planned.atom()
                        && atom.atom_role() == DefinitionAtomRole::RuntimeRecord
                })
                .copied()
                .ok_or(StrongCallableBodyFingerprintError::MissingRuntimeScanAtom {
                    body,
                    atom: planned.atom(),
                })
        })
        .collect()
}

pub(super) fn runtime_scan_fingerprint_atoms(
    object: &[u8],
    member: &crate::link_object::VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongCallableRuntimeScanPlanV1,
    ranges: &[crate::link_object::VerifiedDefinitionAtomRangeV1],
    closure: &crate::link_object::VerifiedCurrentConeStrongRelocationClosureV1,
    requirements: &VerifiedObjectDefinitionRequirementSetV1,
    body: PersistentCallableBodyId,
) -> Result<Vec<AssociatedFingerprintAtom>, StrongCallableBodyFingerprintError> {
    let mut output = Vec::with_capacity(plan.atoms().len());
    for (index, (planned, range)) in plan.atoms().iter().zip(ranges).enumerate() {
        let (section_role, file_start, file_end) =
            atom_file_range(member, *range).map_err(|_| {
                StrongCallableBodyFingerprintError::InvalidRuntimeScanAtomRange {
                    body,
                    atom: planned.atom(),
                }
            })?;
        if section_role != BuiltinObjectSectionRoleV1::ReadOnlyData {
            return Err(
                StrongCallableBodyFingerprintError::RuntimeScanSectionMismatch {
                    body,
                    atom: planned.atom(),
                    actual: section_role,
                },
            );
        }
        let bytes = checked_runtime_scan_bytes(object, file_start, file_end, body, planned.atom())?;
        let (expected_bytes, expected_relocations) = expected_runtime_scan_atom(plan, index)
            .ok_or(StrongCallableBodyFingerprintError::RuntimeScanPlanTree {
                body,
                atom: planned.atom(),
            })?;
        if bytes != expected_bytes {
            return Err(
                StrongCallableBodyFingerprintError::RuntimeScanBytesMismatch {
                    body,
                    atom: planned.atom(),
                },
            );
        }
        let relocations = canonicalize_relocations_with_associated_atoms(
            bytes,
            member,
            planned.atom(),
            closure,
            requirements,
            ranges,
        )
        .map_err(|kind| StrongCallableBodyFingerprintError::Relocation { body, kind })?;
        if relocations.len() != expected_relocations.len()
            || !relocations
                .iter()
                .zip(expected_relocations)
                .all(|(actual, (offset, target))| {
                    actual.is_owning_associated_unsigned64(
                        offset,
                        target,
                        DefinitionAtomRole::RuntimeRecord,
                        0,
                    )
                })
        {
            return Err(
                StrongCallableBodyFingerprintError::RuntimeScanRelocationMismatch {
                    body,
                    atom: planned.atom(),
                },
            );
        }
        let bytes = normalize_atom_bytes(bytes, &relocations)
            .map_err(|kind| StrongCallableBodyFingerprintError::Relocation { body, kind })?;
        output.push(AssociatedFingerprintAtom {
            atom: planned.atom(),
            role: DefinitionAtomRole::RuntimeRecord,
            bytes,
            relocations,
        });
    }
    Ok(output)
}

fn checked_runtime_scan_bytes(
    object: &[u8],
    file_start: u64,
    file_end: u64,
    body: PersistentCallableBodyId,
    atom: scoop_identity::ObjectDefinitionAtomId,
) -> Result<&[u8], StrongCallableBodyFingerprintError> {
    let start = usize::try_from(file_start).map_err(|_| {
        StrongCallableBodyFingerprintError::InvalidRuntimeScanAtomRange { body, atom }
    })?;
    let end = usize::try_from(file_end).map_err(|_| {
        StrongCallableBodyFingerprintError::InvalidRuntimeScanAtomRange { body, atom }
    })?;
    object
        .get(start..end)
        .ok_or(StrongCallableBodyFingerprintError::InvalidRuntimeScanAtomRange { body, atom })
}

pub(super) fn expected_runtime_scan_atom(
    plan: &StrongCallableRuntimeScanPlanV1,
    index: usize,
) -> Option<ExpectedRuntimeScanAtom> {
    let planned = plan.atoms().get(index)?;
    let mut words = Vec::new();
    let mut relocations = Vec::new();
    match planned.scan() {
        RefScan::None => return None,
        RefScan::References(offsets) => {
            if offsets.is_empty() {
                return None;
            }
            words.push(offsets.len() as u64);
            words.extend_from_slice(offsets);
        }
        RefScan::Sequence(parts) => {
            let materialized = parts
                .iter()
                .filter(|part| part.contains_reference())
                .collect::<Vec<_>>();
            let total_children = materialized.iter().try_fold(0usize, |total, part| {
                total.checked_add(runtime_scan_node_count(part)?)
            })?;
            let mut cursor = index.checked_sub(total_children)?;
            words.push(u64::MAX - 1);
            words.push(materialized.len() as u64);
            for child in materialized {
                let count = runtime_scan_node_count(child)?;
                let child_root = cursor.checked_add(count.checked_sub(1)?)?;
                let child_atom = plan.atoms().get(child_root)?;
                if child_atom.scan() != child {
                    return None;
                }
                relocations.push((words.len() as u64 * 8, child_atom.atom()));
                words.push(0);
                cursor = cursor.checked_add(count)?;
            }
            if cursor != index {
                return None;
            }
        }
        RefScan::Array {
            length_offset,
            first_element_offset,
            stride,
            element,
        } => {
            let count = runtime_scan_node_count(element.as_ref_scan())?;
            let child_root = index.checked_sub(1)?;
            let child_start = index.checked_sub(count)?;
            let child_atom = plan.atoms().get(child_root)?;
            if child_start.checked_add(count)? != index
                || child_atom.scan() != element.as_ref_scan()
            {
                return None;
            }
            words.extend([
                u64::MAX,
                *length_offset,
                *first_element_offset,
                stride.get(),
                0,
            ]);
            relocations.push((32, child_atom.atom()));
        }
    }
    let bytes = words
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect::<Vec<_>>();
    Some((bytes, relocations))
}

fn runtime_scan_node_count(scan: &RefScan) -> Option<usize> {
    match scan {
        RefScan::None => Some(0),
        RefScan::References(offsets) => Some(usize::from(!offsets.is_empty())),
        RefScan::Sequence(parts) => {
            let children = parts.iter().try_fold(0usize, |total, part| {
                total.checked_add(runtime_scan_node_count(part)?)
            })?;
            children.checked_add(usize::from(children != 0))
        }
        RefScan::Array { element, .. } => {
            runtime_scan_node_count(element.as_ref_scan())?.checked_add(1)
        }
    }
}
