use std::fmt;

use super::SafepointRegistrationDataV1;
use crate::link_object::{RegistrationAbiV1, StackmapRecordFingerprintV1};
use scoop_identity::{DigestNodeId, PersistentSafepointSiteId};
use scoop_wire::{HashError, RuntimeEncodeError, RuntimeEncoder};

const SAFEPOINT_REGISTRATION_RECORD_KIND: u32 = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongSafepointFingerprintV1 {
    site: PersistentSafepointSiteId,

    stackmap_node: DigestNodeId,
    stackmap: StackmapRecordFingerprintV1,

    registration: RegistrationAbiV1,
}

impl VerifiedStrongSafepointFingerprintV1 {
    pub const fn site(self) -> PersistentSafepointSiteId {
        self.site
    }

    pub const fn stackmap_node(self) -> DigestNodeId {
        self.stackmap_node
    }

    pub const fn stackmap(self) -> StackmapRecordFingerprintV1 {
        self.stackmap
    }

    pub const fn registration(self) -> RegistrationAbiV1 {
        self.registration
    }
}

/// Stackmap fingerprints and registration ABIs from already verified records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongSafepointFingerprintSetV1 {
    registrations: SafepointRegistrationDataV1,
    fingerprints: Vec<VerifiedStrongSafepointFingerprintV1>,
}

impl VerifiedStrongSafepointFingerprintSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.registrations.producer()
    }

    pub const fn registrations(&self) -> &SafepointRegistrationDataV1 {
        &self.registrations
    }

    pub fn fingerprints(&self) -> &[VerifiedStrongSafepointFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_safepoint_fingerprints_v1(
    registrations: SafepointRegistrationDataV1,
) -> Result<VerifiedStrongSafepointFingerprintSetV1, StrongSafepointFingerprintError> {
    let planned = registrations.plan().registrations();
    let stackmaps = registrations.stackmaps().records();
    let mut fingerprints = Vec::with_capacity(planned.len());
    for plan in planned {
        let index = stackmaps
            .binary_search_by_key(&plan.site(), |record| {
                record.normalized().canonical().site()
            })
            .map_err(|_| StrongSafepointFingerprintError::MissingStackmap(plan.site()))?;
        let stackmap = &stackmaps[index];
        let stackmap_fingerprint = stackmap.normalized().fingerprint();
        let registration = RegistrationAbiV1::from_owner(
            plan.definition_owner(),
            scoop_lir::RegistrationTableV1::Safepoint,
        )
        .map_err(|source| StrongSafepointFingerprintError::Hash {
            site: plan.site(),
            source,
        })?;
        fingerprints.push(VerifiedStrongSafepointFingerprintV1 {
            site: plan.site(),
            stackmap_node: plan.normalized_stackmap_fingerprint_node(),
            stackmap: stackmap_fingerprint,

            registration,
        });
    }

    Ok(VerifiedStrongSafepointFingerprintSetV1 {
        registrations,
        fingerprints,
    })
}

pub(in crate::link_object) fn runtime_encode_safepoint_record_v1(
    encoder: &mut RuntimeEncoder,
    plan: scoop_lir::StrongSafepointRegistrationPlanV1,
    normalized_stackmap: &[u8; 32],
) -> Result<(), RuntimeEncodeError> {
    encoder.u32(SAFEPOINT_REGISTRATION_RECORD_KIND)?;
    crate::link_object::registration_identity::runtime_encode_registration_identity(
        encoder,
        plan.site().as_array(),
        plan.definition_owner(),
    )?;
    encoder.u64(plan.safepoint().get())?;
    encoder.u32(plan.role().tag())?;
    encoder.u32(plan.root_pair_count())?;
    encoder.fixed(plan.owner().as_array())?;
    encoder.fixed(normalized_stackmap)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongSafepointFingerprintError {
    MissingStackmap(PersistentSafepointSiteId),
    Hash {
        site: PersistentSafepointSiteId,
        source: HashError,
    },
}

impl fmt::Display for StrongSafepointFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute strong safepoint fingerprints: {self:?}"
        )
    }
}

impl std::error::Error for StrongSafepointFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hash { source, .. } => Some(source),
            _ => None,
        }
    }
}
