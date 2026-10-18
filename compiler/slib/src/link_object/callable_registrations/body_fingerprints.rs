use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{DefinitionAtomRole, DigestKind, DigestNodeId, PersistentCallableBodyId};
use scoop_wire::{HashError, domain_separated_runtime_hash};

use super::CallableRegistrationDataV1;
use super::object_definition::{
    CanonicalAssociatedObjectAtomV1, CanonicalDigestInputV1, CanonicalObjectRelocationV1,
    ObjectDefinitionFingerprintInputV1, ObjectDefinitionLeafWithAssociatedAtomsInputV1,
    ObjectDefinitionRelocationFailureV1, canonicalize_relocations_with_associated_atoms,
};
use crate::SlibMemberId;
use crate::link_object::atom_range::atom_file_range;
use crate::link_object::{
    CanonicalUndefinedSymbolRequirementSetV1,
    FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    FinalizedUndefinedSymbolRequirementPartitionsV1, ObjectDefinitionFingerprintV1,
    ScoopLirObjectCandidateV1, ScoopStackmapSetV1, VerifiedObjectDefinitionRequirementSetV1,
};

mod associated;
mod error;

pub use error::StrongCallableBodyFingerprintError;

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

/// Actual callable code and associated data fingerprints with their inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongCallableBodyObjectFingerprintSetV1 {
    registrations: CallableRegistrationDataV1,
    stackmaps: ScoopStackmapSetV1,
    undefined_requirements: VerifiedObjectDefinitionRequirementSetV1,
    fingerprints: Vec<VerifiedStrongCallableBodyObjectFingerprintV1>,
}

impl VerifiedStrongCallableBodyObjectFingerprintSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.registrations.producer()
    }

    pub const fn registrations(&self) -> &CallableRegistrationDataV1 {
        &self.registrations
    }

    pub const fn stackmaps(&self) -> &ScoopStackmapSetV1 {
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
    registrations: CallableRegistrationDataV1,
    stackmaps: ScoopStackmapSetV1,
    undefined_requirements: CanonicalUndefinedSymbolRequirementSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongCallableBodyObjectFingerprintSetV1, StrongCallableBodyFingerprintError> {
    compute_strong_callable_body_object_fingerprints_inner_v1(
        registrations,
        stackmaps,
        undefined_requirements.into(),
        scoop_objects,
    )
}

pub fn compute_cross_cone_strong_callable_body_object_fingerprints_v1(
    registrations: CallableRegistrationDataV1,
    stackmaps: ScoopStackmapSetV1,
    undefined_requirements: FinalizedUndefinedSymbolRequirementPartitionsV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongCallableBodyObjectFingerprintSetV1, StrongCallableBodyFingerprintError> {
    compute_strong_callable_body_object_fingerprints_inner_v1(
        registrations,
        stackmaps,
        undefined_requirements.into(),
        scoop_objects,
    )
}

pub fn compute_layout_strong_callable_body_object_fingerprints_v1(
    registrations: CallableRegistrationDataV1,
    stackmaps: ScoopStackmapSetV1,
    undefined_requirements: FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongCallableBodyObjectFingerprintSetV1, StrongCallableBodyFingerprintError> {
    compute_strong_callable_body_object_fingerprints_inner_v1(
        registrations,
        stackmaps,
        undefined_requirements.into(),
        scoop_objects,
    )
}

fn compute_strong_callable_body_object_fingerprints_inner_v1(
    registrations: CallableRegistrationDataV1,
    stackmaps: ScoopStackmapSetV1,
    undefined_requirements: VerifiedObjectDefinitionRequirementSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongCallableBodyObjectFingerprintSetV1, StrongCallableBodyFingerprintError> {
    let builtins = registrations.patch_sites().builtins();
    let objects = scoop_objects
        .iter()
        .map(|object| (object.member(), object.bytes()))
        .collect::<BTreeMap<_, _>>();
    let stackmap_inputs = stackmap_inputs(&stackmaps)?;
    let mut fingerprints = Vec::with_capacity(registrations.registrations().len());
    for plan in registrations.plan().registrations() {
        let member = builtins
            .member_plan()
            .member_for_definition(plan.body_definition_plan())
            .ok_or(
                StrongCallableBodyFingerprintError::MissingDefinitionAssignment {
                    body: plan.body(),
                },
            )?;
        let verified_member = builtins
            .member(member)
            .ok_or(StrongCallableBodyFingerprintError::MissingObject(member))?;
        let definition = verified_member
            .definitions()
            .definition(plan.body_definition_plan())
            .ok_or(StrongCallableBodyFingerprintError::MissingBodyDefinition {
                body: plan.body(),
            })?;
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
        let (_, file_start, file_end) = atom_file_range(verified_member, atom).map_err(|_| {
            StrongCallableBodyFingerprintError::InvalidBodyPrimaryAtomRange { body: plan.body() }
        })?;
        let object = objects
            .get(&member)
            .copied()
            .ok_or(StrongCallableBodyFingerprintError::MissingObject(member))?;
        let bytes = checked_atom_bytes(object, file_start, file_end, plan.body())?;
        let ranges = associated::ranges(definition);
        let associated_ranges = definition.atoms();
        let direct_inputs = stackmap_inputs
            .get(&plan.body())
            .into_iter()
            .flat_map(|inputs| inputs.iter())
            .map(|(node, digest)| CanonicalDigestInputV1 {
                kind: DigestKind::StackmapRecord,
                node: *node,
                digest: *digest,
            })
            .collect::<Vec<_>>();
        let relocations = canonicalize_relocations_with_associated_atoms(
            bytes,
            verified_member,
            plan.body_primary_atom(),
            builtins.strong_relocations(),
            &undefined_requirements,
            associated_ranges,
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
        let mut fingerprint_atoms = associated::fingerprint_atoms(
            object,
            verified_member,
            &ranges,
            associated_ranges,
            builtins.strong_relocations(),
            &undefined_requirements,
            plan.body(),
        )?;
        fingerprint_atoms.sort_unstable_by_key(|atom| atom.atom);
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
        registrations,
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

fn stackmap_inputs(
    stackmaps: &ScoopStackmapSetV1,
) -> Result<
    BTreeMap<PersistentCallableBodyId, BTreeMap<DigestNodeId, [u8; 32]>>,
    StrongCallableBodyFingerprintError,
> {
    let mut inputs: BTreeMap<_, BTreeMap<_, _>> = BTreeMap::new();
    for record in stackmaps.records() {
        let canonical = record.normalized().canonical();
        let key = scoop_identity::DigestNodeKey::stackmap_record(canonical.site());
        let node = DigestNodeId::from_key(&key).map_err(|source| {
            StrongCallableBodyFingerprintError::StackmapNodeIdentity {
                site: canonical.site(),
                source,
            }
        })?;
        inputs
            .entry(canonical.owner())
            .or_default()
            .insert(node, *record.normalized().fingerprint().as_array());
    }
    Ok(inputs)
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
