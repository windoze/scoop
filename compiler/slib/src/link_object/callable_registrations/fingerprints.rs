use std::fmt;

use scoop_identity::{DigestNodeId, PersistentCallableBodyId};
use scoop_wire::{HashError, domain_separated_runtime_hash};

use super::object_definition::{CanonicalObjectRelocationV1, ObjectDefinitionFingerprintInputV1};
use super::physical::validate_objects;
use super::record::DESCRIPTOR_SIZE;
use super::{StrongCallableRegistrationValidationError, VerifiedStrongCallableRegistrationSetV1};
use crate::SlibMemberId;
use crate::link_object::{ObjectDefinitionFingerprintV1, ScoopLirObjectCandidateV1};

const OBJECT_DEFINITION_DOMAIN: &str = "scoop-object-definition-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongCallableRegistrationObjectFingerprintV1 {
    body: PersistentCallableBodyId,
    node: DigestNodeId,
    fingerprint: ObjectDefinitionFingerprintV1,
}

impl VerifiedStrongCallableRegistrationObjectFingerprintV1 {
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

/// Canonical registration-object leaves derived from exact provisional bytes
/// and the verified callable-entry relocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongCallableRegistrationObjectFingerprintSetV1 {
    registrations: VerifiedStrongCallableRegistrationSetV1,
    fingerprints: Vec<VerifiedStrongCallableRegistrationObjectFingerprintV1>,
}

impl VerifiedStrongCallableRegistrationObjectFingerprintSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.registrations.producer()
    }

    pub const fn registrations(&self) -> &VerifiedStrongCallableRegistrationSetV1 {
        &self.registrations
    }

    pub fn fingerprints(&self) -> &[VerifiedStrongCallableRegistrationObjectFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_callable_registration_object_fingerprints_v1(
    registrations: VerifiedStrongCallableRegistrationSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<
    VerifiedStrongCallableRegistrationObjectFingerprintSetV1,
    StrongCallableRegistrationObjectFingerprintError,
> {
    let objects = validate_objects(registrations.patch_sites().builtins(), scoop_objects)
        .map_err(StrongCallableRegistrationObjectFingerprintError::ObjectValidation)?;
    if registrations.registrations().len() != registrations.plan().registrations().len() {
        return Err(StrongCallableRegistrationObjectFingerprintError::ProofCoverageMismatch);
    }

    let mut fingerprints = Vec::with_capacity(registrations.registrations().len());
    for (verified, plan) in registrations
        .registrations()
        .iter()
        .zip(registrations.plan().registrations())
    {
        if verified.body() != plan.body() {
            return Err(StrongCallableRegistrationObjectFingerprintError::ProofCoverageMismatch);
        }
        let object = objects.get(&verified.member()).copied().ok_or(
            StrongCallableRegistrationObjectFingerprintError::MissingObject(verified.member()),
        )?;
        let bytes = registration_record_bytes(object, verified.checked_offset(), plan.body())?;
        let fingerprint =
            registration_object_fingerprint(bytes, plan.body()).map_err(|source| {
                StrongCallableRegistrationObjectFingerprintError::Hash {
                    body: plan.body(),
                    source,
                }
            })?;
        fingerprints.push(VerifiedStrongCallableRegistrationObjectFingerprintV1 {
            body: plan.body(),
            node: plan.registration_object_node(),
            fingerprint,
        });
    }

    Ok(VerifiedStrongCallableRegistrationObjectFingerprintSetV1 {
        registrations,
        fingerprints,
    })
}

fn registration_record_bytes(
    object: &[u8],
    checked_offset: u64,
    body: PersistentCallableBodyId,
) -> Result<&[u8], StrongCallableRegistrationObjectFingerprintError> {
    let start = usize::try_from(checked_offset)
        .map_err(|_| StrongCallableRegistrationObjectFingerprintError::RecordRange(body))?;
    let end = start
        .checked_add(DESCRIPTOR_SIZE)
        .ok_or(StrongCallableRegistrationObjectFingerprintError::RecordRange(body))?;
    object
        .get(start..end)
        .ok_or(StrongCallableRegistrationObjectFingerprintError::RecordRange(body))
}

fn registration_object_fingerprint(
    bytes: &[u8],
    body: PersistentCallableBodyId,
) -> Result<ObjectDefinitionFingerprintV1, HashError> {
    let relocations = [CanonicalObjectRelocationV1::callable_entry(body)];
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
pub enum StrongCallableRegistrationObjectFingerprintError {
    ObjectValidation(StrongCallableRegistrationValidationError),
    ProofCoverageMismatch,
    MissingObject(SlibMemberId),
    RecordRange(PersistentCallableBodyId),
    Hash {
        body: PersistentCallableBodyId,
        source: HashError,
    },
}

impl fmt::Display for StrongCallableRegistrationObjectFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute strong callable registration object fingerprints: {self:?}"
        )
    }
}

impl std::error::Error for StrongCallableRegistrationObjectFingerprintError {
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
