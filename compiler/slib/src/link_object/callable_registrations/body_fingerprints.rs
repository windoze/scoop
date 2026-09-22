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
    FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    FinalizedUndefinedSymbolRequirementPartitionsV1, ObjectDefinitionFingerprintV1,
    ScoopLirObjectCandidateV1, VerifiedObjectDefinitionRequirementSetV1,
    VerifiedScoopLirStackmapSetV1,
};

mod constants;
mod error;
mod runtime_scans;

pub use error::StrongCallableBodyFingerprintError;
use runtime_scans::*;

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
    undefined_requirements: VerifiedObjectDefinitionRequirementSetV1,
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
        self.undefined_requirements.legacy()
    }

    pub const fn object_definition_requirements(
        &self,
    ) -> &VerifiedObjectDefinitionRequirementSetV1 {
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
    compute_strong_callable_body_object_fingerprints_inner_v1(
        registration_objects,
        stackmaps,
        undefined_requirements.into(),
        scoop_objects,
    )
}

pub fn compute_cross_cone_strong_callable_body_object_fingerprints_v1(
    registration_objects: VerifiedStrongCallableRegistrationObjectFingerprintSetV1,
    stackmaps: VerifiedScoopLirStackmapSetV1,
    undefined_requirements: FinalizedUndefinedSymbolRequirementPartitionsV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongCallableBodyObjectFingerprintSetV1, StrongCallableBodyFingerprintError> {
    compute_strong_callable_body_object_fingerprints_inner_v1(
        registration_objects,
        stackmaps,
        undefined_requirements.into(),
        scoop_objects,
    )
}

pub fn compute_layout_strong_callable_body_object_fingerprints_v1(
    registration_objects: VerifiedStrongCallableRegistrationObjectFingerprintSetV1,
    stackmaps: VerifiedScoopLirStackmapSetV1,
    undefined_requirements: FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongCallableBodyObjectFingerprintSetV1, StrongCallableBodyFingerprintError> {
    compute_strong_callable_body_object_fingerprints_inner_v1(
        registration_objects,
        stackmaps,
        undefined_requirements.into(),
        scoop_objects,
    )
}

fn compute_strong_callable_body_object_fingerprints_inner_v1(
    registration_objects: VerifiedStrongCallableRegistrationObjectFingerprintSetV1,
    stackmaps: VerifiedScoopLirStackmapSetV1,
    undefined_requirements: VerifiedObjectDefinitionRequirementSetV1,
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
        let constant_ranges = constants::ranges(definition);
        let mut associated_ranges = runtime_scan_ranges.clone();
        associated_ranges.extend_from_slice(&constant_ranges);
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
            &associated_ranges,
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
        let mut fingerprint_atoms = runtime_scan_fingerprint_atoms(
            object,
            verified_member,
            runtime_scan_plan,
            &runtime_scan_ranges,
            builtins.strong_relocations(),
            &undefined_requirements,
            plan.body(),
        )?;
        fingerprint_atoms.extend(constants::fingerprint_atoms(
            object,
            verified_member,
            &constant_ranges,
            &associated_ranges,
            builtins.strong_relocations(),
            &undefined_requirements,
            plan.body(),
        )?);
        let associated_atoms = fingerprint_atoms
            .iter()
            .map(|atom| CanonicalAssociatedObjectAtomV1 {
                atom: atom.atom,
                role: atom.role,
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

struct AssociatedFingerprintAtom {
    atom: scoop_identity::ObjectDefinitionAtomId,
    role: DefinitionAtomRole,
    bytes: Vec<u8>,
    relocations: Vec<CanonicalObjectRelocationV1>,
}

fn validate_undefined_requirements(
    builtins: &crate::link_object::VerifiedBuiltinObjectStrongRelocationSetV1,
    requirements: &VerifiedObjectDefinitionRequirementSetV1,
) -> Result<(), StrongCallableBodyFingerprintError> {
    if !requirements.matches_strong_closure(builtins.strong_relocations()) {
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

#[cfg(test)]
mod tests;
