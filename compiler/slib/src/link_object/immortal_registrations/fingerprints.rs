use std::fmt;

use scoop_identity::{
    DigestNodeId, PersistentExactTypeId, PersistentImmortalObjectId, StrongDefinitionEntity,
    StrongDefinitionRole,
};
use scoop_lir::ImmortalObjectTypeRegistrationRefV1;
use scoop_wire::{HashError, domain_separated_runtime_hash};

use super::physical::validate_objects;
use super::record::DESCRIPTOR_SIZE;
use super::{
    StrongImmortalObjectRegistrationValidationError, VerifiedStrongImmortalObjectRegistrationSetV1,
};
use crate::SlibMemberId;
use crate::link_object::callable_registrations::object_definition::{
    CanonicalObjectRelocationV1, ObjectDefinitionFingerprintInputV1,
};
use crate::link_object::{
    FinalUndefinedSymbolRequirementV1, ObjectDefinitionFingerprintV1, ScoopLirObjectCandidateV1,
    StrongDefinitionOwnerV1,
};

const OBJECT_DEFINITION_DOMAIN: &str = "scoop-object-definition-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongImmortalObjectRegistrationObjectFingerprintV1 {
    object: PersistentImmortalObjectId,
    node: DigestNodeId,
    fingerprint: ObjectDefinitionFingerprintV1,
}

impl VerifiedStrongImmortalObjectRegistrationObjectFingerprintV1 {
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

/// Canonical registration-object leaves derived from exact provisional bytes
/// and both verified typed relocations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1 {
    registrations: VerifiedStrongImmortalObjectRegistrationSetV1,
    fingerprints: Vec<VerifiedStrongImmortalObjectRegistrationObjectFingerprintV1>,
}

impl VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.registrations.producer()
    }

    pub const fn registrations(&self) -> &VerifiedStrongImmortalObjectRegistrationSetV1 {
        &self.registrations
    }

    pub fn fingerprints(&self) -> &[VerifiedStrongImmortalObjectRegistrationObjectFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_immortal_object_registration_object_fingerprints_v1(
    registrations: VerifiedStrongImmortalObjectRegistrationSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<
    VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
    StrongImmortalObjectRegistrationObjectFingerprintError,
> {
    let objects = validate_objects(registrations.patch_sites().builtins(), scoop_objects)
        .map_err(StrongImmortalObjectRegistrationObjectFingerprintError::ObjectValidation)?;
    if registrations.registrations().len() != registrations.plan().registrations().len() {
        return Err(StrongImmortalObjectRegistrationObjectFingerprintError::ProofCoverageMismatch);
    }

    let mut fingerprints = Vec::with_capacity(registrations.registrations().len());
    for (verified, plan) in registrations
        .registrations()
        .iter()
        .zip(registrations.plan().registrations())
    {
        if verified.object() != plan.object() {
            return Err(
                StrongImmortalObjectRegistrationObjectFingerprintError::ProofCoverageMismatch,
            );
        }
        let object = objects.get(&verified.member()).copied().ok_or(
            StrongImmortalObjectRegistrationObjectFingerprintError::MissingObject(
                verified.member(),
            ),
        )?;
        let bytes = registration_record_bytes(object, verified.checked_offset(), plan.object())?;
        let fingerprint = registration_object_fingerprint(bytes, *plan).map_err(|source| {
            StrongImmortalObjectRegistrationObjectFingerprintError::Hash {
                object: plan.object(),
                source,
            }
        })?;
        fingerprints.push(
            VerifiedStrongImmortalObjectRegistrationObjectFingerprintV1 {
                object: plan.object(),
                node: plan.registration_object_node(),
                fingerprint,
            },
        );
    }

    Ok(
        VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1 {
            registrations,
            fingerprints,
        },
    )
}

fn registration_record_bytes(
    object: &[u8],
    checked_offset: u64,
    identity: PersistentImmortalObjectId,
) -> Result<&[u8], StrongImmortalObjectRegistrationObjectFingerprintError> {
    let start = usize::try_from(checked_offset).map_err(|_| {
        StrongImmortalObjectRegistrationObjectFingerprintError::RecordRange(identity)
    })?;
    let end = start
        .checked_add(DESCRIPTOR_SIZE)
        .ok_or(StrongImmortalObjectRegistrationObjectFingerprintError::RecordRange(identity))?;
    object
        .get(start..end)
        .ok_or(StrongImmortalObjectRegistrationObjectFingerprintError::RecordRange(identity))
}

fn registration_object_fingerprint(
    bytes: &[u8],
    plan: scoop_lir::StrongImmortalObjectRegistrationPlanV1,
) -> Result<ObjectDefinitionFingerprintV1, HashError> {
    let object_owner = StrongDefinitionOwnerV1::new(
        StrongDefinitionEntity::immortal_object(plan.object()),
        StrongDefinitionRole::ImmortalObject,
    )
    .expect("immortal objects are valid strong definition owners");
    let object_relocation = CanonicalObjectRelocationV1::unsigned64(
        152,
        FinalUndefinedSymbolRequirementV1::IntraConeStrong {
            owner: object_owner,
        },
    );
    let type_owner = type_registration_owner(plan.type_registration());
    let type_registration = match plan.semantic().type_registration_ref() {
        ImmortalObjectTypeRegistrationRefV1::Local(_) => CanonicalObjectRelocationV1::unsigned64(
            176,
            FinalUndefinedSymbolRequirementV1::IntraConeStrong { owner: type_owner },
        ),
        ImmortalObjectTypeRegistrationRefV1::DependencyExternal { provider, .. } => {
            CanonicalObjectRelocationV1::unsigned64(
                176,
                FinalUndefinedSymbolRequirementV1::DependencyStrong {
                    provider,
                    owner: type_owner,
                },
            )
        }
    };
    let relocations = [object_relocation, type_registration];
    domain_separated_runtime_hash(
        OBJECT_DEFINITION_DOMAIN,
        &ObjectDefinitionFingerprintInputV1 {
            bytes,
            relocations: &relocations,
            direct_inputs: &[],
        },
    )
    .map(|digest| ObjectDefinitionFingerprintV1::from_array(*digest.as_array()))
}

fn type_registration_owner(exact_type: PersistentExactTypeId) -> StrongDefinitionOwnerV1 {
    StrongDefinitionOwnerV1::new(
        StrongDefinitionEntity::exact_type(exact_type),
        StrongDefinitionRole::TypeRegistration,
    )
    .expect("type registrations are valid strong definition owners")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongImmortalObjectRegistrationObjectFingerprintError {
    ObjectValidation(StrongImmortalObjectRegistrationValidationError),
    ProofCoverageMismatch,
    MissingObject(SlibMemberId),
    RecordRange(PersistentImmortalObjectId),
    Hash {
        object: PersistentImmortalObjectId,
        source: HashError,
    },
}

impl fmt::Display for StrongImmortalObjectRegistrationObjectFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute strong immortal-object registration object fingerprints: {self:?}"
        )
    }
}

impl std::error::Error for StrongImmortalObjectRegistrationObjectFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ObjectValidation(source) => Some(source),
            Self::Hash { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
