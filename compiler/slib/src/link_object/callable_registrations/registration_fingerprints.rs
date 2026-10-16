use std::fmt;

use scoop_identity::{DigestNodeId, PersistentCallableBodyId};
use scoop_lir::CanonicalCallableAbisV1;
use scoop_wire::{HashError, RuntimeEncodeError, RuntimeEncoder};

use super::VerifiedStrongCallableBodyObjectFingerprintSetV1;
use crate::link_object::{
    CallableDefinitionAbiV1, ObjectDefinitionFingerprintV1, RegistrationAbiV1,
};

const CALLABLE_REGISTRATION_RECORD_KIND: u32 = 6;
const OWN_CALLABLE_ENTRY_ROLE: u32 = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongCallableFingerprintV1 {
    body: PersistentCallableBodyId,

    body_definition_node: DigestNodeId,
    body_definition: ObjectDefinitionFingerprintV1,
    definition: CallableDefinitionAbiV1,

    registration: RegistrationAbiV1,
}

impl VerifiedStrongCallableFingerprintV1 {
    pub const fn body(self) -> PersistentCallableBodyId {
        self.body
    }

    pub const fn body_definition_node(self) -> DigestNodeId {
        self.body_definition_node
    }

    pub const fn body_definition(self) -> ObjectDefinitionFingerprintV1 {
        self.body_definition
    }

    pub const fn definition(self) -> CallableDefinitionAbiV1 {
        self.definition
    }

    pub const fn registration(self) -> RegistrationAbiV1 {
        self.registration
    }
}

/// Actual body fingerprints and shared ABIs of verified callable records.
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
    canonical: &CanonicalCallableAbisV1,
) -> Result<VerifiedStrongCallableFingerprintSetV1, StrongCallableFingerprintError> {
    let registrations = body_objects.registrations();
    let planned = registrations.plan().registrations();
    let mut fingerprints = Vec::with_capacity(planned.len());
    for (plan, body_definition) in planned.iter().zip(body_objects.fingerprints()) {
        let body = plan.body();
        let canonical = canonical
            .get(body)
            .ok_or(StrongCallableFingerprintError::CanonicalBodyMismatch { body })?;
        let definition = crate::link_object::odr_member_fingerprints::callable_body(*canonical);
        let registration = RegistrationAbiV1::from_owner(
            plan.definition_owner(),
            scoop_lir::RegistrationTableV1::Callable,
        )
        .map_err(|source| StrongCallableFingerprintError::Hash { body, source })?;
        fingerprints.push(VerifiedStrongCallableFingerprintV1 {
            body,

            body_definition_node: body_definition.node(),
            body_definition: body_definition.fingerprint(),
            definition,

            registration,
        });
    }

    Ok(VerifiedStrongCallableFingerprintSetV1 {
        body_objects,
        fingerprints,
    })
}

pub(in crate::link_object) fn runtime_encode_callable_record_v1(
    encoder: &mut RuntimeEncoder,
    plan: scoop_lir::StrongCallableRegistrationPlanV1,
    body_definition: &[u8; 32],
) -> Result<(), RuntimeEncodeError> {
    encoder.u32(CALLABLE_REGISTRATION_RECORD_KIND)?;
    crate::link_object::registration_identity::runtime_encode_registration_identity(
        encoder,
        plan.body().as_array(),
        plan.definition_owner(),
    )?;
    encoder.fixed(body_definition)?;
    encoder.u32(OWN_CALLABLE_ENTRY_ROLE)?;
    encoder.fixed(plan.body().as_array())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongCallableFingerprintError {
    CanonicalBodyMismatch {
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
