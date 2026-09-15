use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{DefinitionAtomRole, DigestKind, DigestNodeId, PersistentCallableBodyId};
use scoop_lir::{DigestInputRefV1, RefScan, StrongCallableRuntimeScanPlanV1};
use scoop_wire::{HashError, domain_separated_runtime_hash};

use super::object_definition::{
    CanonicalAssociatedObjectAtomV1, CanonicalDigestInputV1, CanonicalObjectRelocationV1,
    ObjectDefinitionFingerprintInputV1, ObjectDefinitionLeafWithAssociatedAtomsInputV1,
    ObjectDefinitionRelocationFailureV1, canonicalize_relocations_with_associated_atoms,
};
use super::physical::{atom_file_range, validate_objects, verified_member};
use super::{
    StrongCallableRegistrationValidationError,
    VerifiedStrongCallableRegistrationObjectFingerprintSetV1,
};
use crate::SlibMemberId;
use crate::link_object::{
    BuiltinObjectSectionRoleV1, CanonicalUndefinedSymbolRequirementSetV1,
    ObjectDefinitionFingerprintV1, ScoopLirObjectCandidateV1, VerifiedScoopLirStackmapSetV1,
};

const OBJECT_DEFINITION_DOMAIN: &str = "scoop-object-definition-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongCallableBodyObjectFingerprintV1 {
    body: PersistentCallableBodyId,
    node: DigestNodeId,
    fingerprint: ObjectDefinitionFingerprintV1,
}

impl VerifiedStrongCallableBodyObjectFingerprintV1 {
    pub const fn body(self) -> PersistentCallableBodyId {
        self.body
    }

    pub const fn node(self) -> DigestNodeId {
        self.node
    }

    pub const fn fingerprint(self) -> ObjectDefinitionFingerprintV1 {
        self.fingerprint
    }
}

/// Callable-body ObjectDefinition leaves bound to their registration,
/// stackmap, undefined-requirement, relocation, and exact-object proofs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongCallableBodyObjectFingerprintSetV1 {
    registration_objects: VerifiedStrongCallableRegistrationObjectFingerprintSetV1,
    stackmaps: VerifiedScoopLirStackmapSetV1,
    undefined_requirements: CanonicalUndefinedSymbolRequirementSetV1,
    fingerprints: Vec<VerifiedStrongCallableBodyObjectFingerprintV1>,
}

impl VerifiedStrongCallableBodyObjectFingerprintSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.registration_objects.producer()
    }

    pub const fn registration_objects(
        &self,
    ) -> &VerifiedStrongCallableRegistrationObjectFingerprintSetV1 {
        &self.registration_objects
    }

    pub const fn stackmaps(&self) -> &VerifiedScoopLirStackmapSetV1 {
        &self.stackmaps
    }

    pub const fn undefined_requirements(&self) -> &CanonicalUndefinedSymbolRequirementSetV1 {
        &self.undefined_requirements
    }

    pub fn fingerprints(&self) -> &[VerifiedStrongCallableBodyObjectFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_callable_body_object_fingerprints_v1(
    registration_objects: VerifiedStrongCallableRegistrationObjectFingerprintSetV1,
    stackmaps: VerifiedScoopLirStackmapSetV1,
    undefined_requirements: CanonicalUndefinedSymbolRequirementSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongCallableBodyObjectFingerprintSetV1, StrongCallableBodyFingerprintError> {
    let registrations = registration_objects.registrations();
    let builtins = registrations.patch_sites().builtins();
    if stackmaps.builtins() != builtins {
        return Err(StrongCallableBodyFingerprintError::ObjectProofMismatch);
    }
    validate_undefined_requirements(builtins, &undefined_requirements)?;
    let objects = validate_objects(builtins, scoop_objects)
        .map_err(StrongCallableBodyFingerprintError::ObjectValidation)?;
    if registrations.registrations().len() != registrations.plan().registrations().len() {
        return Err(StrongCallableBodyFingerprintError::ProofCoverageMismatch);
    }

    let stackmap_inputs = stackmap_inputs(&stackmaps)?;
    let mut fingerprints = Vec::with_capacity(registrations.registrations().len());
    for (verified, plan) in registrations
        .registrations()
        .iter()
        .zip(registrations.plan().registrations())
    {
        if verified.body() != plan.body() {
            return Err(StrongCallableBodyFingerprintError::ProofCoverageMismatch);
        }
        let member = builtins
            .member_plan()
            .member_for_definition(plan.body_definition_plan())
            .ok_or(
                StrongCallableBodyFingerprintError::MissingDefinitionAssignment {
                    body: plan.body(),
                },
            )?;
        let verified_member = verified_member(builtins, member)
            .map_err(StrongCallableBodyFingerprintError::ObjectValidation)?;
        let definition = verified_member
            .definitions()
            .definition(plan.body_definition_plan())
            .ok_or(StrongCallableBodyFingerprintError::MissingBodyDefinition {
                body: plan.body(),
            })?;
        if definition.primary_atom() != plan.body_primary_atom() {
            return Err(
                StrongCallableBodyFingerprintError::BodyPrimaryAtomMismatch { body: plan.body() },
            );
        }
        let atom = definition
            .atoms()
            .iter()
            .find(|atom| {
                atom.atom() == plan.body_primary_atom()
                    && atom.atom_role() == DefinitionAtomRole::Primary
            })
            .copied()
            .ok_or(StrongCallableBodyFingerprintError::MissingBodyPrimaryAtom {
                body: plan.body(),
            })?;
        let (section_role, file_start, file_end) =
            atom_file_range(verified_member, atom).map_err(|_| {
                StrongCallableBodyFingerprintError::InvalidBodyPrimaryAtomRange {
                    body: plan.body(),
                }
            })?;
        if section_role != BuiltinObjectSectionRoleV1::Text {
            return Err(
                StrongCallableBodyFingerprintError::BodyPrimarySectionMismatch {
                    body: plan.body(),
                    actual: section_role,
                },
            );
        }
        let object = objects
            .get(&member)
            .copied()
            .ok_or(StrongCallableBodyFingerprintError::MissingObject(member))?;
        let bytes = checked_atom_bytes(object, file_start, file_end, plan.body())?;
        let runtime_scan_plan = registrations
            .plan()
            .runtime_scans()
            .callable(plan.body())
            .ok_or(StrongCallableBodyFingerprintError::MissingRuntimeScanPlan {
                body: plan.body(),
            })?;
        let runtime_scan_ranges =
            exact_runtime_scan_ranges(definition, runtime_scan_plan, plan.body())?;
        let direct_inputs = exact_stackmap_inputs(
            registrations.patch_sites().digest_plan(),
            plan.body_definition_node(),
            plan.body(),
            &stackmap_inputs,
        )?;
        let relocations = canonicalize_relocations_with_associated_atoms(
            bytes,
            verified_member,
            plan.body_primary_atom(),
            builtins.strong_relocations(),
            &undefined_requirements,
            &runtime_scan_ranges,
        )
        .map_err(|kind| StrongCallableBodyFingerprintError::Relocation {
            body: plan.body(),
            kind,
        })?;
        let normalized_bytes = normalize_atom_bytes(bytes, &relocations).map_err(|kind| {
            StrongCallableBodyFingerprintError::Relocation {
                body: plan.body(),
                kind,
            }
        })?;
        let runtime_scan_atoms = runtime_scan_fingerprint_atoms(
            object,
            verified_member,
            runtime_scan_plan,
            &runtime_scan_ranges,
            builtins.strong_relocations(),
            &undefined_requirements,
            plan.body(),
        )?;
        let associated_atoms = runtime_scan_atoms
            .iter()
            .map(|atom| CanonicalAssociatedObjectAtomV1 {
                atom: atom.atom,
                role: DefinitionAtomRole::RuntimeRecord,
                bytes: &atom.bytes,
                relocations: &atom.relocations,
            })
            .collect::<Vec<_>>();
        let fingerprint = domain_separated_runtime_hash(
            OBJECT_DEFINITION_DOMAIN,
            &ObjectDefinitionLeafWithAssociatedAtomsInputV1 {
                primary: ObjectDefinitionFingerprintInputV1 {
                    bytes: &normalized_bytes,
                    relocations: &relocations,
                    direct_inputs: &direct_inputs,
                },
                associated_atoms: &associated_atoms,
            },
        )
        .map(|digest| ObjectDefinitionFingerprintV1::from_array(*digest.as_array()))
        .map_err(|source| StrongCallableBodyFingerprintError::Hash {
            body: plan.body(),
            source,
        })?;
        fingerprints.push(VerifiedStrongCallableBodyObjectFingerprintV1 {
            body: plan.body(),
            node: plan.body_definition_node(),
            fingerprint,
        });
    }

    Ok(VerifiedStrongCallableBodyObjectFingerprintSetV1 {
        registration_objects,
        stackmaps,
        undefined_requirements,
        fingerprints,
    })
}

struct RuntimeScanFingerprintAtom {
    atom: scoop_identity::ObjectDefinitionAtomId,
    bytes: Vec<u8>,
    relocations: Vec<CanonicalObjectRelocationV1>,
}

type ExpectedRuntimeScanAtom = (Vec<u8>, Vec<(u64, scoop_identity::ObjectDefinitionAtomId)>);

fn exact_runtime_scan_ranges(
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

fn runtime_scan_fingerprint_atoms(
    object: &[u8],
    member: &crate::link_object::VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongCallableRuntimeScanPlanV1,
    ranges: &[crate::link_object::VerifiedDefinitionAtomRangeV1],
    closure: &crate::link_object::VerifiedCurrentConeStrongRelocationClosureV1,
    requirements: &CanonicalUndefinedSymbolRequirementSetV1,
    body: PersistentCallableBodyId,
) -> Result<Vec<RuntimeScanFingerprintAtom>, StrongCallableBodyFingerprintError> {
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
        output.push(RuntimeScanFingerprintAtom {
            atom: planned.atom(),
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

fn expected_runtime_scan_atom(
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

fn validate_undefined_requirements(
    builtins: &crate::link_object::VerifiedBuiltinObjectStrongRelocationSetV1,
    requirements: &CanonicalUndefinedSymbolRequirementSetV1,
) -> Result<(), StrongCallableBodyFingerprintError> {
    if !requirements.matches_strong_closure(builtins) {
        return Err(StrongCallableBodyFingerprintError::UndefinedRequirementProofMismatch);
    }
    Ok(())
}

fn stackmap_inputs(
    stackmaps: &VerifiedScoopLirStackmapSetV1,
) -> Result<
    BTreeMap<DigestNodeId, (PersistentCallableBodyId, [u8; 32])>,
    StrongCallableBodyFingerprintError,
> {
    let mut inputs = BTreeMap::new();
    for record in stackmaps.records() {
        let canonical = record.normalized().canonical();
        let key = scoop_identity::DigestNodeKey::stackmap_record(canonical.site());
        let node = DigestNodeId::from_key(&key).map_err(|source| {
            StrongCallableBodyFingerprintError::StackmapNodeIdentity {
                site: canonical.site(),
                source,
            }
        })?;
        inputs.insert(
            node,
            (
                canonical.owner(),
                *record.normalized().fingerprint().as_array(),
            ),
        );
    }
    Ok(inputs)
}

fn exact_stackmap_inputs(
    digest_plan: &scoop_lir::StrongDigestFinalizationPlanV1,
    node: DigestNodeId,
    body: PersistentCallableBodyId,
    stackmaps: &BTreeMap<DigestNodeId, (PersistentCallableBodyId, [u8; 32])>,
) -> Result<Vec<CanonicalDigestInputV1>, StrongCallableBodyFingerprintError> {
    let body_node = digest_plan
        .nodes()
        .iter()
        .find(|candidate| candidate.id() == node)
        .ok_or(StrongCallableBodyFingerprintError::MissingBodyDigestNode { body })?;
    let expected = stackmaps
        .iter()
        .filter(|(_, (owner, _))| *owner == body)
        .map(|(node, _)| DigestInputRefV1::StackmapRecord(*node))
        .collect::<Vec<_>>();
    if let Some(input) = body_node
        .direct_inputs()
        .iter()
        .find(|input| !matches!(input, DigestInputRefV1::StackmapRecord(_)))
    {
        return Err(
            StrongCallableBodyFingerprintError::UnsupportedBodyDirectInput {
                body,
                kind: input.kind(),
            },
        );
    }
    if body_node.direct_inputs() != expected {
        return Err(StrongCallableBodyFingerprintError::BodyDirectInputMismatch { body });
    }
    body_node
        .direct_inputs()
        .iter()
        .map(|input| {
            let DigestInputRefV1::StackmapRecord(node) = input else {
                return Err(
                    StrongCallableBodyFingerprintError::UnsupportedBodyDirectInput {
                        body,
                        kind: input.kind(),
                    },
                );
            };
            let (_, digest) = stackmaps.get(node).ok_or(
                StrongCallableBodyFingerprintError::MissingStackmapInput { body, node: *node },
            )?;
            Ok(CanonicalDigestInputV1 {
                kind: DigestKind::StackmapRecord,
                node: *node,
                digest: *digest,
            })
        })
        .collect()
}

fn checked_atom_bytes(
    object: &[u8],
    file_start: u64,
    file_end: u64,
    body: PersistentCallableBodyId,
) -> Result<&[u8], StrongCallableBodyFingerprintError> {
    let start = usize::try_from(file_start)
        .map_err(|_| StrongCallableBodyFingerprintError::BodyRange { body })?;
    let end = usize::try_from(file_end)
        .map_err(|_| StrongCallableBodyFingerprintError::BodyRange { body })?;
    object
        .get(start..end)
        .ok_or(StrongCallableBodyFingerprintError::BodyRange { body })
}

fn normalize_atom_bytes(
    bytes: &[u8],
    relocations: &[CanonicalObjectRelocationV1],
) -> Result<Vec<u8>, ObjectDefinitionRelocationFailureV1> {
    let mut normalized = bytes.to_vec();
    for relocation in relocations {
        relocation.normalize_bytes(&mut normalized)?;
    }
    Ok(normalized)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongCallableBodyFingerprintError {
    ObjectProofMismatch,
    UndefinedRequirementProofMismatch,
    ObjectValidation(StrongCallableRegistrationValidationError),
    ProofCoverageMismatch,
    MissingDefinitionAssignment {
        body: PersistentCallableBodyId,
    },
    MissingBodyDefinition {
        body: PersistentCallableBodyId,
    },
    BodyPrimaryAtomMismatch {
        body: PersistentCallableBodyId,
    },
    MissingBodyPrimaryAtom {
        body: PersistentCallableBodyId,
    },
    MissingRuntimeScanPlan {
        body: PersistentCallableBodyId,
    },
    RuntimeScanAtomSetMismatch {
        body: PersistentCallableBodyId,
        expected: Vec<scoop_identity::ObjectDefinitionAtomId>,
        actual: Vec<scoop_identity::ObjectDefinitionAtomId>,
    },
    MissingRuntimeScanAtom {
        body: PersistentCallableBodyId,
        atom: scoop_identity::ObjectDefinitionAtomId,
    },
    InvalidRuntimeScanAtomRange {
        body: PersistentCallableBodyId,
        atom: scoop_identity::ObjectDefinitionAtomId,
    },
    RuntimeScanSectionMismatch {
        body: PersistentCallableBodyId,
        atom: scoop_identity::ObjectDefinitionAtomId,
        actual: BuiltinObjectSectionRoleV1,
    },
    RuntimeScanBytesMismatch {
        body: PersistentCallableBodyId,
        atom: scoop_identity::ObjectDefinitionAtomId,
    },
    RuntimeScanRelocationMismatch {
        body: PersistentCallableBodyId,
        atom: scoop_identity::ObjectDefinitionAtomId,
    },
    RuntimeScanPlanTree {
        body: PersistentCallableBodyId,
        atom: scoop_identity::ObjectDefinitionAtomId,
    },
    InvalidBodyPrimaryAtomRange {
        body: PersistentCallableBodyId,
    },
    BodyPrimarySectionMismatch {
        body: PersistentCallableBodyId,
        actual: BuiltinObjectSectionRoleV1,
    },
    MissingObject(SlibMemberId),
    BodyRange {
        body: PersistentCallableBodyId,
    },
    MissingBodyDigestNode {
        body: PersistentCallableBodyId,
    },
    BodyDirectInputMismatch {
        body: PersistentCallableBodyId,
    },
    UnsupportedBodyDirectInput {
        body: PersistentCallableBodyId,
        kind: DigestKind,
    },
    MissingStackmapInput {
        body: PersistentCallableBodyId,
        node: DigestNodeId,
    },
    StackmapNodeIdentity {
        site: scoop_identity::PersistentSafepointSiteId,
        source: HashError,
    },
    Relocation {
        body: PersistentCallableBodyId,
        kind: ObjectDefinitionRelocationFailureV1,
    },
    Hash {
        body: PersistentCallableBodyId,
        source: HashError,
    },
}

impl fmt::Display for StrongCallableBodyFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute strong callable body object fingerprints: {self:?}"
        )
    }
}

impl std::error::Error for StrongCallableBodyFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ObjectValidation(source) => Some(source),
            Self::StackmapNodeIdentity { source, .. } | Self::Hash { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
