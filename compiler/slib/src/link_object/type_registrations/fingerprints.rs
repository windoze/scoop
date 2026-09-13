use std::fmt;

use scoop_identity::{DigestNodeId, PersistentExactTypeId};
use scoop_wire::{HashError, domain_separated_runtime_hash};

use super::physical::validate_objects;
use super::record::DESCRIPTOR_SIZE;
use super::{StrongTypeRegistrationValidationError, VerifiedStrongTypeRegistrationSetV1};
use crate::SlibMemberId;
use crate::link_object::callable_registrations::object_definition::{
    CanonicalObjectRelocationV1, ObjectDefinitionFingerprintInputV1,
};
use crate::link_object::{ObjectDefinitionFingerprintV1, ScoopLirObjectCandidateV1};

const OBJECT_DEFINITION_DOMAIN: &str = "scoop-object-definition-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongTypeRegistrationObjectFingerprintV1 {
    exact_type: PersistentExactTypeId,
    node: DigestNodeId,
    fingerprint: ObjectDefinitionFingerprintV1,
}

impl VerifiedStrongTypeRegistrationObjectFingerprintV1 {
    pub const fn exact_type(self) -> PersistentExactTypeId {
        self.exact_type
    }

    pub const fn node(self) -> DigestNodeId {
        self.node
    }

    pub const fn fingerprint(self) -> ObjectDefinitionFingerprintV1 {
        self.fingerprint
    }
}

/// Canonical registration-object leaves derived from exact provisional bytes
/// and the verified TypeDescriptor relocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongTypeRegistrationObjectFingerprintSetV1 {
    registrations: VerifiedStrongTypeRegistrationSetV1,
    fingerprints: Vec<VerifiedStrongTypeRegistrationObjectFingerprintV1>,
}

impl VerifiedStrongTypeRegistrationObjectFingerprintSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.registrations.producer()
    }

    pub const fn registrations(&self) -> &VerifiedStrongTypeRegistrationSetV1 {
        &self.registrations
    }

    pub fn fingerprints(&self) -> &[VerifiedStrongTypeRegistrationObjectFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_type_registration_object_fingerprints_v1(
    registrations: VerifiedStrongTypeRegistrationSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<
    VerifiedStrongTypeRegistrationObjectFingerprintSetV1,
    StrongTypeRegistrationObjectFingerprintError,
> {
    let objects = validate_objects(registrations.patch_sites().builtins(), scoop_objects)
        .map_err(StrongTypeRegistrationObjectFingerprintError::ObjectValidation)?;
    if registrations.registrations().len() != registrations.plan().registrations().len() {
        return Err(StrongTypeRegistrationObjectFingerprintError::ProofCoverageMismatch);
    }

    let mut fingerprints = Vec::with_capacity(registrations.registrations().len());
    for (verified, plan) in registrations
        .registrations()
        .iter()
        .zip(registrations.plan().registrations())
    {
        if verified.exact_type() != plan.exact_type() {
            return Err(StrongTypeRegistrationObjectFingerprintError::ProofCoverageMismatch);
        }
        let object = objects.get(&verified.member()).copied().ok_or(
            StrongTypeRegistrationObjectFingerprintError::MissingObject(verified.member()),
        )?;
        let bytes =
            registration_record_bytes(object, verified.checked_offset(), plan.exact_type())?;
        let fingerprint =
            registration_object_fingerprint(bytes, plan.exact_type()).map_err(|source| {
                StrongTypeRegistrationObjectFingerprintError::Hash {
                    exact_type: plan.exact_type(),
                    source,
                }
            })?;
        fingerprints.push(VerifiedStrongTypeRegistrationObjectFingerprintV1 {
            exact_type: plan.exact_type(),
            node: plan.registration_object_node(),
            fingerprint,
        });
    }

    Ok(VerifiedStrongTypeRegistrationObjectFingerprintSetV1 {
        registrations,
        fingerprints,
    })
}

fn registration_record_bytes(
    object: &[u8],
    checked_offset: u64,
    exact_type: PersistentExactTypeId,
) -> Result<&[u8], StrongTypeRegistrationObjectFingerprintError> {
    let start = usize::try_from(checked_offset)
        .map_err(|_| StrongTypeRegistrationObjectFingerprintError::RecordRange(exact_type))?;
    let end = start.checked_add(DESCRIPTOR_SIZE).ok_or(
        StrongTypeRegistrationObjectFingerprintError::RecordRange(exact_type),
    )?;
    object
        .get(start..end)
        .ok_or(StrongTypeRegistrationObjectFingerprintError::RecordRange(
            exact_type,
        ))
}

fn registration_object_fingerprint(
    bytes: &[u8],
    exact_type: PersistentExactTypeId,
) -> Result<ObjectDefinitionFingerprintV1, HashError> {
    let relocations = [CanonicalObjectRelocationV1::type_descriptor(exact_type)];
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongTypeRegistrationObjectFingerprintError {
    ObjectValidation(StrongTypeRegistrationValidationError),
    ProofCoverageMismatch,
    MissingObject(SlibMemberId),
    RecordRange(PersistentExactTypeId),
    Hash {
        exact_type: PersistentExactTypeId,
        source: HashError,
    },
}

impl fmt::Display for StrongTypeRegistrationObjectFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute strong type registration object fingerprints: {self:?}"
        )
    }
}

impl std::error::Error for StrongTypeRegistrationObjectFingerprintError {
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
