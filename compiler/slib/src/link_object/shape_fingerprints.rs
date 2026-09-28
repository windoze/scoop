//! Content fingerprints of physical ODR shapes from the existing object index.

use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{DefinitionAtomRole, ObjectDefinitionAtomId, ObjectDefinitionPlanId};
use scoop_lir::{CanonicalShapeLirDefinitionV1, CanonicalShapeLirDefinitionsV1};
use scoop_wire::{HashError, domain_separated_runtime_hash};

use super::callable_registrations::object_definition::{
    CanonicalAssociatedObjectAtomV1, CanonicalObjectRelocationV1,
    ObjectDefinitionFingerprintInputV1, ObjectDefinitionLeafWithAssociatedAtomsInputV1,
    canonicalize_relocations_with_associated_atoms,
};
use super::safepoint_registrations::physical::atom_file_range;
use super::{
    ObjectDefinitionFingerprintV1, ObjectDefinitionRelocationFailureV1, OdrMemberFingerprintV1,
    VerifiedCurrentConeStrongRelocationClosureV1, VerifiedObjectDefinitionRequirementSetV1,
};
use crate::SlibMemberId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OdrShapeFingerprintV1 {
    canonical: CanonicalShapeLirDefinitionV1,
    object: ObjectDefinitionFingerprintV1,
    member: OdrMemberFingerprintV1,
}

impl OdrShapeFingerprintV1 {
    pub const fn canonical(self) -> CanonicalShapeLirDefinitionV1 {
        self.canonical
    }
    pub const fn object(self) -> ObjectDefinitionFingerprintV1 {
        self.object
    }
    pub const fn member(self) -> OdrMemberFingerprintV1 {
        self.member
    }
}

pub(super) fn compute(
    canonical: &CanonicalShapeLirDefinitionsV1,
    closure: &VerifiedCurrentConeStrongRelocationClosureV1,
    requirements: &VerifiedObjectDefinitionRequirementSetV1,
    objects: &BTreeMap<SlibMemberId, &[u8]>,
) -> Result<Vec<OdrShapeFingerprintV1>, OdrShapeFingerprintError> {
    let definitions = closure
        .members()
        .iter()
        .flat_map(|member| {
            member
                .definitions()
                .definitions()
                .iter()
                .map(move |definition| (definition.definition(), (member, definition)))
        })
        .collect::<BTreeMap<_, _>>();
    canonical
        .definitions()
        .iter()
        .filter(|canonical| {
            !matches!(
                canonical.role(),
                scoop_identity::OdrMemberRole::ImmortalObject
                    | scoop_identity::OdrMemberRole::StaticStorage
                    | scoop_identity::OdrMemberRole::InitializationCell
                    | scoop_identity::OdrMemberRole::InitializationDescriptor
            )
        })
        .map(|canonical| {
            let (member, definition) = definitions.get(&canonical.definition()).copied().ok_or(
                OdrShapeFingerprintError::MissingDefinition(canonical.definition()),
            )?;
            let bytes = objects
                .get(&member.member())
                .copied()
                .ok_or(OdrShapeFingerprintError::MissingObject(member.member()))?;
            let mut atoms = Vec::with_capacity(definition.atoms().len());
            for range in definition.atoms() {
                let (_, start, end) = atom_file_range(member, *range)
                    .map_err(|_| OdrShapeFingerprintError::Range(range.atom()))?;
                let start = usize::try_from(start)
                    .map_err(|_| OdrShapeFingerprintError::Range(range.atom()))?;
                let end = usize::try_from(end)
                    .map_err(|_| OdrShapeFingerprintError::Range(range.atom()))?;
                let bytes = bytes
                    .get(start..end)
                    .ok_or(OdrShapeFingerprintError::Range(range.atom()))?;
                let relocations = canonicalize_relocations_with_associated_atoms(
                    bytes,
                    member,
                    range.atom(),
                    closure,
                    requirements,
                    definition.atoms(),
                )
                .map_err(|kind| OdrShapeFingerprintError::Relocation {
                    atom: range.atom(),
                    kind,
                })?;
                let mut normalized = bytes.to_vec();
                for relocation in &relocations {
                    relocation
                        .normalize_bytes(&mut normalized)
                        .map_err(|kind| OdrShapeFingerprintError::Relocation {
                            atom: range.atom(),
                            kind,
                        })?;
                }
                atoms.push(Atom {
                    id: range.atom(),
                    role: range.atom_role(),
                    bytes: normalized,
                    relocations,
                });
            }
            let primary = atoms
                .iter()
                .find(|atom| atom.id == canonical.primary_atom())
                .ok_or(OdrShapeFingerprintError::MissingDefinition(
                    canonical.definition(),
                ))?;
            let mut associated = atoms
                .iter()
                .filter(|atom| atom.id != primary.id)
                .map(|atom| CanonicalAssociatedObjectAtomV1 {
                    atom: atom.id,
                    role: atom.role,
                    bytes: &atom.bytes,
                    relocations: &atom.relocations,
                })
                .collect::<Vec<_>>();
            associated.sort_unstable_by_key(|atom| atom.atom);
            let object = domain_separated_runtime_hash(
                "scoop-object-definition-v1",
                &ObjectDefinitionLeafWithAssociatedAtomsInputV1 {
                    primary: ObjectDefinitionFingerprintInputV1 {
                        bytes: &primary.bytes,
                        relocations: &primary.relocations,
                        direct_inputs: &[],
                    },
                    associated_atoms: &associated,
                },
            )
            .map(|digest| ObjectDefinitionFingerprintV1::from_array(*digest.as_array()))
            .map_err(OdrShapeFingerprintError::Hash)?;
            let member = super::odr_member_fingerprints::shape_definition(*canonical, object)
                .map_err(OdrShapeFingerprintError::Hash)?;
            Ok(OdrShapeFingerprintV1 {
                canonical: *canonical,
                object,
                member,
            })
        })
        .collect()
}

struct Atom {
    id: ObjectDefinitionAtomId,
    role: DefinitionAtomRole,
    bytes: Vec<u8>,
    relocations: Vec<CanonicalObjectRelocationV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OdrShapeFingerprintError {
    MissingDefinition(ObjectDefinitionPlanId),
    MissingObject(SlibMemberId),
    Range(ObjectDefinitionAtomId),
    Relocation {
        atom: ObjectDefinitionAtomId,
        kind: ObjectDefinitionRelocationFailureV1,
    },
    Hash(HashError),
}

impl fmt::Display for OdrShapeFingerprintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid ODR shape content: {self:?}")
    }
}
impl std::error::Error for OdrShapeFingerprintError {}
