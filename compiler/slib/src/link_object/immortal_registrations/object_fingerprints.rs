use std::fmt;

use scoop_identity::{DigestNodeId, PersistentImmortalObjectId};
use scoop_wire::{HashError, domain_separated_runtime_hash};

use super::fingerprints::VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1;
use super::physical::validate_objects;
use crate::SlibMemberId;
use crate::link_object::callable_registrations::object_definition::{
    ObjectDefinitionFingerprintInputV1, canonicalize_relocations,
};
use crate::link_object::{
    BuiltinObjectSectionRoleV1, CanonicalUndefinedSymbolRequirementSetV1,
    FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    FinalizedUndefinedSymbolRequirementPartitionsV1, ImmortalObjectRegistrationRelocationFailureV1,
    ObjectDefinitionFingerprintV1, ObjectDefinitionRelocationFailureV1, ScoopLirObjectCandidateV1,
    StrongImmortalObjectRegistrationValidationError, VerifiedObjectDefinitionRequirementSetV1,
};

mod validation;
use validation::validate_immortal_object;

const OBJECT_DEFINITION_DOMAIN: &str = "scoop-object-definition-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongImmortalObjectDefinitionFingerprintV1 {
    object: PersistentImmortalObjectId,
    node: DigestNodeId,
    fingerprint: ObjectDefinitionFingerprintV1,
}

impl VerifiedStrongImmortalObjectDefinitionFingerprintV1 {
    pub const fn object(self) -> PersistentImmortalObjectId {
        self.object
    }

    pub const fn node(self) -> DigestNodeId {
        self.node
    }

    pub const fn fingerprint(self) -> ObjectDefinitionFingerprintV1 {
        self.fingerprint
    }
}

/// Exact immutable-object leaves bound to registration, relocation,
/// undefined-requirement, and provisional object-byte proofs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongImmortalObjectDefinitionFingerprintSetV1 {
    registration_objects: VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
    undefined_requirements: VerifiedObjectDefinitionRequirementSetV1,
    fingerprints: Vec<VerifiedStrongImmortalObjectDefinitionFingerprintV1>,
}

impl VerifiedStrongImmortalObjectDefinitionFingerprintSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.registration_objects.producer()
    }

    pub const fn registration_objects(
        &self,
    ) -> &VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1 {
        &self.registration_objects
    }

    pub const fn undefined_requirements(&self) -> &CanonicalUndefinedSymbolRequirementSetV1 {
        self.undefined_requirements.legacy()
    }

    pub const fn object_definition_requirements(
        &self,
    ) -> &VerifiedObjectDefinitionRequirementSetV1 {
        &self.undefined_requirements
    }

    pub fn fingerprints(&self) -> &[VerifiedStrongImmortalObjectDefinitionFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_immortal_object_definition_fingerprints_v1(
    registration_objects: VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
    undefined_requirements: CanonicalUndefinedSymbolRequirementSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<
    VerifiedStrongImmortalObjectDefinitionFingerprintSetV1,
    StrongImmortalObjectDefinitionFingerprintError,
> {
    compute_strong_immortal_object_definition_fingerprints_inner_v1(
        registration_objects,
        undefined_requirements.into(),
        scoop_objects,
    )
}

pub fn compute_cross_cone_strong_immortal_object_definition_fingerprints_v1(
    registration_objects: VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
    undefined_requirements: FinalizedUndefinedSymbolRequirementPartitionsV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<
    VerifiedStrongImmortalObjectDefinitionFingerprintSetV1,
    StrongImmortalObjectDefinitionFingerprintError,
> {
    compute_strong_immortal_object_definition_fingerprints_inner_v1(
        registration_objects,
        undefined_requirements.into(),
        scoop_objects,
    )
}

pub fn compute_layout_strong_immortal_object_definition_fingerprints_v1(
    registration_objects: VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
    undefined_requirements: FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<
    VerifiedStrongImmortalObjectDefinitionFingerprintSetV1,
    StrongImmortalObjectDefinitionFingerprintError,
> {
    compute_strong_immortal_object_definition_fingerprints_inner_v1(
        registration_objects,
        undefined_requirements.into(),
        scoop_objects,
    )
}

fn compute_strong_immortal_object_definition_fingerprints_inner_v1(
    registration_objects: VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
    undefined_requirements: VerifiedObjectDefinitionRequirementSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<
    VerifiedStrongImmortalObjectDefinitionFingerprintSetV1,
    StrongImmortalObjectDefinitionFingerprintError,
> {
    let registrations = registration_objects.registrations();
    let builtins = registrations.patch_sites().builtins();
    if !undefined_requirements.matches_strong_closure(builtins.strong_relocations()) {
        return Err(
            StrongImmortalObjectDefinitionFingerprintError::UndefinedRequirementProofMismatch,
        );
    }
    let objects = validate_objects(builtins, scoop_objects)
        .map_err(StrongImmortalObjectDefinitionFingerprintError::ObjectValidation)?;
    let verified = registrations.registrations();
    let planned = registrations.plan().registrations();
    let registration_fingerprints = registration_objects.fingerprints();
    if verified.len() != planned.len() || verified.len() != registration_fingerprints.len() {
        return Err(StrongImmortalObjectDefinitionFingerprintError::ProofCoverageMismatch);
    }

    let mut fingerprints = Vec::with_capacity(verified.len());
    for ((verified, plan), registration_fingerprint) in
        verified.iter().zip(planned).zip(registration_fingerprints)
    {
        let object = plan.object();
        if verified.object() != object
            || registration_fingerprint.object() != object
            || registration_fingerprint.node() != plan.registration_object_node()
        {
            return Err(
                StrongImmortalObjectDefinitionFingerprintError::RegistrationObjectMismatch {
                    object,
                },
            );
        }
        let validated =
            validate_immortal_object(builtins, &undefined_requirements, &objects, *plan)?;
        let relocations = canonicalize_relocations(
            validated.bytes,
            validated.member,
            plan.object_primary_atom(),
            builtins.strong_relocations(),
            &undefined_requirements,
        )
        .map_err(
            |kind| StrongImmortalObjectDefinitionFingerprintError::Relocation { object, kind },
        )?;
        let mut normalized = validated.bytes.to_vec();
        for relocation in &relocations {
            relocation
                .normalize_bytes(&mut normalized)
                .map_err(
                    |kind| StrongImmortalObjectDefinitionFingerprintError::Relocation {
                        object,
                        kind,
                    },
                )?;
        }
        let fingerprint = domain_separated_runtime_hash(
            OBJECT_DEFINITION_DOMAIN,
            &ObjectDefinitionFingerprintInputV1 {
                bytes: &normalized,
                relocations: &relocations,
                direct_inputs: &[],
            },
        )
        .map(|digest| ObjectDefinitionFingerprintV1::from_array(*digest.as_array()))
        .map_err(
            |source| StrongImmortalObjectDefinitionFingerprintError::Hash { object, source },
        )?;
        fingerprints.push(VerifiedStrongImmortalObjectDefinitionFingerprintV1 {
            object,
            node: plan.object_definition_node(),
            fingerprint,
        });
    }

    Ok(VerifiedStrongImmortalObjectDefinitionFingerprintSetV1 {
        registration_objects,
        undefined_requirements,
        fingerprints,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImmortalObjectByteFailureV1 {
    HeaderTooShort,
    ByteMismatch {
        offset: u64,
        expected: u8,
        actual: u8,
    },
    LengthOverflow,
    ExtentMismatch,
    InvalidUtf8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongImmortalObjectDefinitionFingerprintError {
    UndefinedRequirementProofMismatch,
    ObjectValidation(StrongImmortalObjectRegistrationValidationError),
    ProofCoverageMismatch,
    RegistrationObjectMismatch {
        object: PersistentImmortalObjectId,
    },
    MissingDefinitionAssignment {
        object: PersistentImmortalObjectId,
    },
    MissingObjectDefinition {
        object: PersistentImmortalObjectId,
    },
    PrimaryAtomMismatch {
        object: PersistentImmortalObjectId,
    },
    MissingPrimaryAtom {
        object: PersistentImmortalObjectId,
    },
    InvalidPrimaryAtomRange {
        object: PersistentImmortalObjectId,
    },
    PrimaryAtomSectionMismatch {
        object: PersistentImmortalObjectId,
        actual: BuiltinObjectSectionRoleV1,
    },
    ObjectSizeMismatch {
        object: PersistentImmortalObjectId,
        expected: u64,
        actual: u64,
    },
    MissingObject(SlibMemberId),
    ObjectRange {
        object: PersistentImmortalObjectId,
    },
    ObjectBytes {
        object: PersistentImmortalObjectId,
        kind: ImmortalObjectByteFailureV1,
    },
    DescriptorRelocation {
        object: PersistentImmortalObjectId,
        kind: ImmortalObjectRegistrationRelocationFailureV1,
    },
    Relocation {
        object: PersistentImmortalObjectId,
        kind: ObjectDefinitionRelocationFailureV1,
    },
    DefinitionIdentity {
        object: PersistentImmortalObjectId,
        source: scoop_identity::ObjectDefinitionIdentityError,
    },
    IdentityHash {
        object: PersistentImmortalObjectId,
        source: HashError,
    },
    Symbol {
        object: PersistentImmortalObjectId,
        source: scoop_identity::PersistentSymbolError,
    },
    Hash {
        object: PersistentImmortalObjectId,
        source: HashError,
    },
}

impl fmt::Display for StrongImmortalObjectDefinitionFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute strong immortal-object definition fingerprints: {self:?}"
        )
    }
}

impl std::error::Error for StrongImmortalObjectDefinitionFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ObjectValidation(source) => Some(source),
            Self::DefinitionIdentity { source, .. } => Some(source),
            Self::IdentityHash { source, .. } | Self::Hash { source, .. } => Some(source),
            Self::Symbol { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
