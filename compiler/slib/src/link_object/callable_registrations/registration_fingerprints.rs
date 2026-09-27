use std::fmt;

use scoop_identity::{DigestKind, DigestNodeId, PersistentCallableBodyId};
use scoop_lir::RegistrationDefinitionOwner;
use scoop_wire::{
    HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, domain_separated_runtime_hash,
};

use super::VerifiedStrongCallableBodyObjectFingerprintSetV1;
use super::object_definition::CanonicalDigestInputV1;
use crate::link_object::{
    ObjectDefinitionFingerprintV1, RegistrationFingerprintV1, StrongRegistrationFingerprintV1,
};

const STRONG_REGISTRATION_DOMAIN: &str = "scoop-strong-registration-v1";
const CALLABLE_REGISTRATION_RECORD_KIND: u32 = 6;
const OWN_CALLABLE_ENTRY_ROLE: u32 = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongCallableFingerprintV1 {
    body: PersistentCallableBodyId,
    registration_object_node: DigestNodeId,
    registration_object: ObjectDefinitionFingerprintV1,
    body_definition_node: DigestNodeId,
    body_definition: ObjectDefinitionFingerprintV1,
    registration_node: DigestNodeId,
    registration: RegistrationFingerprintV1,
}

impl VerifiedStrongCallableFingerprintV1 {
    pub const fn body(self) -> PersistentCallableBodyId {
        self.body
    }

    pub const fn registration_object_node(self) -> DigestNodeId {
        self.registration_object_node
    }

    pub const fn registration_object(self) -> ObjectDefinitionFingerprintV1 {
        self.registration_object
    }

    pub const fn body_definition_node(self) -> DigestNodeId {
        self.body_definition_node
    }

    pub const fn body_definition(self) -> ObjectDefinitionFingerprintV1 {
        self.body_definition
    }

    pub const fn registration_node(self) -> DigestNodeId {
        self.registration_node
    }

    pub const fn registration(self) -> RegistrationFingerprintV1 {
        self.registration
    }
}

/// Final Strong or ODR registration fingerprints from the actual callable
/// registration and body ObjectDefinition leaves.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongCallableFingerprintSetV1 {
    body_objects: VerifiedStrongCallableBodyObjectFingerprintSetV1,
    fingerprints: Vec<VerifiedStrongCallableFingerprintV1>,
}

impl VerifiedStrongCallableFingerprintSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.body_objects.producer()
    }

    pub const fn body_objects(&self) -> &VerifiedStrongCallableBodyObjectFingerprintSetV1 {
        &self.body_objects
    }

    pub fn fingerprints(&self) -> &[VerifiedStrongCallableFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_callable_fingerprints_v1(
    body_objects: VerifiedStrongCallableBodyObjectFingerprintSetV1,
) -> Result<VerifiedStrongCallableFingerprintSetV1, StrongCallableFingerprintError> {
    let registration_objects = body_objects.registration_objects();
    let registrations = registration_objects.registrations();
    let verified = registrations.registrations();
    let planned = registrations.plan().registrations();
    let registration_fingerprints = registration_objects.fingerprints();
    let body_fingerprints = body_objects.fingerprints();
    if verified.len() != planned.len()
        || verified.len() != registration_fingerprints.len()
        || verified.len() != body_fingerprints.len()
    {
        return Err(StrongCallableFingerprintError::ProofCoverageMismatch);
    }

    let mut fingerprints = Vec::with_capacity(verified.len());
    for (((verified, plan), registration_object), body_definition) in verified
        .iter()
        .zip(planned)
        .zip(registration_fingerprints)
        .zip(body_fingerprints)
    {
        let body = plan.body();
        if verified.body() != body
            || registration_object.body() != body
            || registration_object.node() != plan.registration_object_node()
        {
            return Err(StrongCallableFingerprintError::RegistrationObjectMismatch { body });
        }
        if body_definition.body() != body || body_definition.node() != plan.body_definition_node() {
            return Err(StrongCallableFingerprintError::BodyObjectMismatch { body });
        }
        let registration = match plan.definition_owner() {
            RegistrationDefinitionOwner::Strong => strong_callable_registration_fingerprint(
                *plan,
                registration_object.node(),
                registration_object.fingerprint(),
                body_definition.node(),
                body_definition.fingerprint(),
            )
            .map(RegistrationFingerprintV1::Strong),
            RegistrationDefinitionOwner::Odr { group, member } => {
                crate::link_object::odr_registration_fingerprints::callable_registration(
                    group,
                    member,
                    *plan,
                    registration_object.fingerprint(),
                )
                .map(RegistrationFingerprintV1::Odr)
            }
        }
        .map_err(|source| StrongCallableFingerprintError::Hash { body, source })?;
        fingerprints.push(VerifiedStrongCallableFingerprintV1 {
            body,
            registration_object_node: registration_object.node(),
            registration_object: registration_object.fingerprint(),
            body_definition_node: body_definition.node(),
            body_definition: body_definition.fingerprint(),
            registration_node: plan.registration_fingerprint_node(),
            registration,
        });
    }

    Ok(VerifiedStrongCallableFingerprintSetV1 {
        body_objects,
        fingerprints,
    })
}

fn strong_callable_registration_fingerprint(
    plan: scoop_lir::StrongCallableRegistrationPlanV1,
    registration_object_node: DigestNodeId,
    registration_object: ObjectDefinitionFingerprintV1,
    body_definition_node: DigestNodeId,
    body_definition: ObjectDefinitionFingerprintV1,
) -> Result<StrongRegistrationFingerprintV1, HashError> {
    let mut direct_inputs = [
        CanonicalDigestInputV1 {
            kind: DigestKind::ObjectDefinition,
            node: registration_object_node,
            digest: *registration_object.as_array(),
        },
        CanonicalDigestInputV1 {
            kind: DigestKind::ObjectDefinition,
            node: body_definition_node,
            digest: *body_definition.as_array(),
        },
    ];
    direct_inputs.sort_unstable_by_key(|input| (input.kind.tag(), input.node));
    domain_separated_runtime_hash(
        STRONG_REGISTRATION_DOMAIN,
        &StrongCallableRegistrationFingerprintInputV1 {
            plan,
            body_definition,
            direct_inputs,
        },
    )
    .map(|digest| StrongRegistrationFingerprintV1::from_array(*digest.as_array()))
}

struct StrongCallableRegistrationFingerprintInputV1 {
    plan: scoop_lir::StrongCallableRegistrationPlanV1,
    body_definition: ObjectDefinitionFingerprintV1,
    direct_inputs: [CanonicalDigestInputV1; 2],
}

impl RuntimeEncode for StrongCallableRegistrationFingerprintInputV1 {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        runtime_encode_callable_record_v1(
            encoder,
            self.plan,
            &[0; 32],
            self.body_definition.as_array(),
        )?;
        encoder.sequence_length(self.direct_inputs.len())?;
        for input in self.direct_inputs {
            input.runtime_encode(encoder)?;
        }
        Ok(())
    }
}

pub(in crate::link_object) fn runtime_encode_callable_record_v1(
    encoder: &mut RuntimeEncoder,
    plan: scoop_lir::StrongCallableRegistrationPlanV1,
    registration: &[u8; 32],
    body_definition: &[u8; 32],
) -> Result<(), RuntimeEncodeError> {
    encoder.u32(CALLABLE_REGISTRATION_RECORD_KIND)?;
    crate::link_object::registration_identity::runtime_encode_registration_identity(
        encoder,
        plan.body().as_array(),
        plan.definition_owner(),
        registration,
    )?;
    encoder.fixed(body_definition)?;
    encoder.u32(OWN_CALLABLE_ENTRY_ROLE)?;
    encoder.fixed(plan.body().as_array())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongCallableFingerprintError {
    ProofCoverageMismatch,
    RegistrationObjectMismatch {
        body: PersistentCallableBodyId,
    },
    BodyObjectMismatch {
        body: PersistentCallableBodyId,
    },
    Hash {
        body: PersistentCallableBodyId,
        source: HashError,
    },
}

impl fmt::Display for StrongCallableFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute strong callable fingerprints: {self:?}"
        )
    }
}

impl std::error::Error for StrongCallableFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hash { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
