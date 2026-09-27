use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{DigestKind, DigestNodeId, PersistentCallableBodyId};
use scoop_lir::RegistrationDefinitionOwner;
use scoop_wire::{HashError, domain_separated_runtime_hash};

use super::object_definition::{
    CanonicalDigestInputV1, ObjectDefinitionFingerprintInputV1,
    ObjectDefinitionRelocationFailureV1, canonicalize_relocations,
};
use super::physical::verified_member;
use super::record::DESCRIPTOR_SIZE;
use super::{
    StrongCallableRegistrationValidationError, VerifiedStrongCallableBodyObjectFingerprintV1,
    VerifiedStrongCallableRegistrationSetV1,
};
use crate::SlibMemberId;
use crate::link_object::{ObjectDefinitionFingerprintV1, VerifiedObjectDefinitionRequirementSetV1};

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

pub(super) fn compute_registration_object_fingerprints(
    registrations: VerifiedStrongCallableRegistrationSetV1,
    bodies: &[VerifiedStrongCallableBodyObjectFingerprintV1],
    objects: &BTreeMap<SlibMemberId, &[u8]>,
    requirements: &VerifiedObjectDefinitionRequirementSetV1,
) -> Result<
    VerifiedStrongCallableRegistrationObjectFingerprintSetV1,
    StrongCallableRegistrationObjectFingerprintError,
> {
    let builtins = registrations.patch_sites().builtins();
    debug_assert_eq!(registrations.registrations().len(), bodies.len());
    let mut fingerprints = Vec::with_capacity(registrations.registrations().len());
    for ((verified, plan), body) in registrations
        .registrations()
        .iter()
        .zip(registrations.plan().registrations())
        .zip(bodies)
    {
        debug_assert_eq!(body.body(), plan.body());
        let object = objects.get(&verified.member()).copied().ok_or(
            StrongCallableRegistrationObjectFingerprintError::MissingObject(verified.member()),
        )?;
        let mut bytes =
            registration_record_bytes(object, verified.checked_offset(), plan.body())?.to_vec();
        let member = verified_member(builtins, verified.member())
            .map_err(StrongCallableRegistrationObjectFingerprintError::ObjectValidation)?;
        let relocations = canonicalize_relocations(
            &bytes,
            member,
            plan.primary_atom(),
            builtins.strong_relocations(),
            requirements,
        )
        .map_err(|source| {
            StrongCallableRegistrationObjectFingerprintError::Relocation {
                body: plan.body(),
                source,
            }
        })?;
        let mut direct_inputs = Vec::new();
        if matches!(
            plan.definition_owner(),
            RegistrationDefinitionOwner::Odr { .. }
        ) {
            bytes[152..184].copy_from_slice(body.fingerprint().as_array());
            direct_inputs.push(CanonicalDigestInputV1 {
                kind: DigestKind::ObjectDefinition,
                node: body.node(),
                digest: *body.fingerprint().as_array(),
            });
        }
        let fingerprint = domain_separated_runtime_hash(
            OBJECT_DEFINITION_DOMAIN,
            &ObjectDefinitionFingerprintInputV1 {
                bytes: &bytes,
                relocations: &relocations,
                direct_inputs: &direct_inputs,
            },
        )
        .map(|digest| ObjectDefinitionFingerprintV1::from_array(*digest.as_array()))
        .map_err(
            |source| StrongCallableRegistrationObjectFingerprintError::Hash {
                body: plan.body(),
                source,
            },
        )?;
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongCallableRegistrationObjectFingerprintError {
    ObjectValidation(StrongCallableRegistrationValidationError),
    MissingObject(SlibMemberId),
    RecordRange(PersistentCallableBodyId),
    Relocation {
        body: PersistentCallableBodyId,
        source: ObjectDefinitionRelocationFailureV1,
    },
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
