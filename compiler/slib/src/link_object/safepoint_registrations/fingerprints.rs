use std::fmt;

use scoop_identity::{DigestKind, DigestNodeId, DigestNodeKey, PersistentSafepointSiteId};
use scoop_lir::RegistrationDefinitionOwner;
use scoop_wire::{
    HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, domain_separated_runtime_hash,
};

use super::physical::validate_objects;
use super::record::{DESCRIPTOR_SIZE, NORMALIZED_STACKMAP_FINGERPRINT_OFFSET};
use super::{StrongSafepointRegistrationValidationError, VerifiedStrongSafepointRegistrationSetV1};
use crate::SlibMemberId;
use crate::link_object::callable_registrations::object_definition::{
    CanonicalDigestInputV1, ObjectDefinitionFingerprintInputV1,
};
use crate::link_object::{
    ObjectDefinitionFingerprintV1, RegistrationFingerprintV1, ScoopLirObjectCandidateV1,
    StackmapRecordFingerprintV1, StrongRegistrationFingerprintV1,
};

const OBJECT_DEFINITION_DOMAIN: &str = "scoop-object-definition-v1";
const STRONG_REGISTRATION_DOMAIN: &str = "scoop-strong-registration-v1";
const SAFEPOINT_REGISTRATION_RECORD_KIND: u32 = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongSafepointFingerprintV1 {
    site: PersistentSafepointSiteId,
    object_definition_node: DigestNodeId,
    object_definition: ObjectDefinitionFingerprintV1,
    stackmap_node: DigestNodeId,
    stackmap: StackmapRecordFingerprintV1,
    registration_node: DigestNodeId,
    registration: RegistrationFingerprintV1,
}

impl VerifiedStrongSafepointFingerprintV1 {
    pub const fn site(self) -> PersistentSafepointSiteId {
        self.site
    }

    pub const fn object_definition_node(self) -> DigestNodeId {
        self.object_definition_node
    }

    pub const fn object_definition(self) -> ObjectDefinitionFingerprintV1 {
        self.object_definition
    }

    pub const fn stackmap_node(self) -> DigestNodeId {
        self.stackmap_node
    }

    pub const fn stackmap(self) -> StackmapRecordFingerprintV1 {
        self.stackmap
    }

    pub const fn registration_node(self) -> DigestNodeId {
        self.registration_node
    }

    pub const fn registration(self) -> RegistrationFingerprintV1 {
        self.registration
    }
}

/// Canonical object and Strong/ODR registration fingerprints from the exact
/// object bytes and the already normalized stackmap record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongSafepointFingerprintSetV1 {
    registrations: VerifiedStrongSafepointRegistrationSetV1,
    fingerprints: Vec<VerifiedStrongSafepointFingerprintV1>,
}

impl VerifiedStrongSafepointFingerprintSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.registrations.producer()
    }

    pub const fn registrations(&self) -> &VerifiedStrongSafepointRegistrationSetV1 {
        &self.registrations
    }

    pub fn fingerprints(&self) -> &[VerifiedStrongSafepointFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_safepoint_fingerprints_v1(
    registrations: VerifiedStrongSafepointRegistrationSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongSafepointFingerprintSetV1, StrongSafepointFingerprintError> {
    let objects = validate_objects(registrations.stackmaps().builtins(), scoop_objects)
        .map_err(StrongSafepointFingerprintError::ObjectValidation)?;
    let verified = registrations.registrations();
    let planned = registrations.plan().registrations();
    let stackmaps = registrations.stackmaps().records();
    if verified.len() != planned.len() || verified.len() != stackmaps.len() {
        return Err(StrongSafepointFingerprintError::ProofCoverageMismatch);
    }

    let mut fingerprints = Vec::with_capacity(verified.len());
    for ((verified, plan), stackmap) in verified.iter().zip(planned).zip(stackmaps) {
        let object = objects.get(&verified.member()).copied().ok_or(
            StrongSafepointFingerprintError::MissingObject(verified.member()),
        )?;
        let mut record =
            registration_record_bytes(object, verified.checked_offset(), plan.site())?.to_vec();
        let stackmap_fingerprint = stackmap.normalized().fingerprint();
        let mut direct_inputs = Vec::new();
        if matches!(
            plan.definition_owner(),
            RegistrationDefinitionOwner::Odr { .. }
        ) {
            record[NORMALIZED_STACKMAP_FINGERPRINT_OFFSET
                ..NORMALIZED_STACKMAP_FINGERPRINT_OFFSET + 32]
                .copy_from_slice(stackmap_fingerprint.as_array());
            direct_inputs.push(CanonicalDigestInputV1 {
                kind: DigestKind::StackmapRecord,
                node: plan.normalized_stackmap_fingerprint_node(),
                digest: *stackmap_fingerprint.as_array(),
            });
        }
        let object_definition =
            relocation_free_object_definition_fingerprint(&record, &direct_inputs).map_err(
                |source| StrongSafepointFingerprintError::Hash {
                    site: plan.site(),
                    kind: SafepointFingerprintKindV1::ObjectDefinition,
                    source,
                },
            )?;
        let object_definition_node =
            DigestNodeId::from_key(&DigestNodeKey::object_definition(plan.primary_atom()))
                .map_err(|source| StrongSafepointFingerprintError::Hash {
                    site: plan.site(),
                    kind: SafepointFingerprintKindV1::ObjectDefinitionNode,
                    source,
                })?;
        let registration = match plan.definition_owner() {
            RegistrationDefinitionOwner::Strong => strong_registration_fingerprint(
                *plan,
                object_definition_node,
                object_definition.as_array(),
                stackmap_fingerprint.as_array(),
            )
            .map(RegistrationFingerprintV1::Strong),
            RegistrationDefinitionOwner::Odr { group, member } => {
                crate::link_object::odr_member_fingerprints::safepoint_registration(
                    group,
                    member,
                    *plan,
                    object_definition_node,
                    object_definition,
                    stackmap_fingerprint,
                )
                .map(RegistrationFingerprintV1::Odr)
            }
        }
        .map_err(|source| StrongSafepointFingerprintError::Hash {
            site: plan.site(),
            kind: SafepointFingerprintKindV1::RegistrationDefinition,
            source,
        })?;
        fingerprints.push(VerifiedStrongSafepointFingerprintV1 {
            site: plan.site(),
            object_definition_node,
            object_definition,
            stackmap_node: plan.normalized_stackmap_fingerprint_node(),
            stackmap: stackmap_fingerprint,
            registration_node: plan.registration_fingerprint_node(),
            registration,
        });
    }

    Ok(VerifiedStrongSafepointFingerprintSetV1 {
        registrations,
        fingerprints,
    })
}

fn registration_record_bytes(
    object: &[u8],
    checked_offset: u64,
    site: PersistentSafepointSiteId,
) -> Result<&[u8], StrongSafepointFingerprintError> {
    let start = usize::try_from(checked_offset)
        .map_err(|_| StrongSafepointFingerprintError::RecordRange(site))?;
    let end = start
        .checked_add(DESCRIPTOR_SIZE)
        .ok_or(StrongSafepointFingerprintError::RecordRange(site))?;
    object
        .get(start..end)
        .ok_or(StrongSafepointFingerprintError::RecordRange(site))
}

fn relocation_free_object_definition_fingerprint(
    bytes: &[u8],
    direct_inputs: &[CanonicalDigestInputV1],
) -> Result<ObjectDefinitionFingerprintV1, HashError> {
    domain_separated_runtime_hash(
        OBJECT_DEFINITION_DOMAIN,
        &ObjectDefinitionFingerprintInputV1 {
            bytes,
            relocations: &[],
            direct_inputs,
        },
    )
    .map(|digest| ObjectDefinitionFingerprintV1::from_array(*digest.as_array()))
}

fn strong_registration_fingerprint(
    plan: scoop_lir::StrongSafepointRegistrationPlanV1,
    object_definition_node: DigestNodeId,
    object_definition: &[u8; 32],
    stackmap: &[u8; 32],
) -> Result<StrongRegistrationFingerprintV1, HashError> {
    let input = StrongSafepointRegistrationFingerprintInput {
        plan,
        direct_inputs: [
            StrongRegistrationDigestInput {
                kind: DigestKind::ObjectDefinition,
                node: object_definition_node,
                digest: object_definition,
            },
            StrongRegistrationDigestInput {
                kind: DigestKind::StackmapRecord,
                node: plan.normalized_stackmap_fingerprint_node(),
                digest: stackmap,
            },
        ],
        normalized_stackmap: stackmap,
    };
    domain_separated_runtime_hash(STRONG_REGISTRATION_DOMAIN, &input)
        .map(|digest| StrongRegistrationFingerprintV1::from_array(*digest.as_array()))
}

struct StrongSafepointRegistrationFingerprintInput<'a> {
    plan: scoop_lir::StrongSafepointRegistrationPlanV1,
    direct_inputs: [StrongRegistrationDigestInput<'a>; 2],
    normalized_stackmap: &'a [u8; 32],
}

impl RuntimeEncode for StrongSafepointRegistrationFingerprintInput<'_> {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        runtime_encode_safepoint_record_v1(encoder, self.plan, &[0; 32], self.normalized_stackmap)?;
        encoder.sequence_length(self.direct_inputs.len())?;
        for input in self.direct_inputs {
            input.runtime_encode(encoder)?;
        }
        Ok(())
    }
}

pub(in crate::link_object) fn runtime_encode_safepoint_record_v1(
    encoder: &mut RuntimeEncoder,
    plan: scoop_lir::StrongSafepointRegistrationPlanV1,
    registration: &[u8; 32],
    normalized_stackmap: &[u8; 32],
) -> Result<(), RuntimeEncodeError> {
    encoder.u32(SAFEPOINT_REGISTRATION_RECORD_KIND)?;
    crate::link_object::registration_identity::runtime_encode_registration_identity(
        encoder,
        plan.site().as_array(),
        plan.definition_owner(),
        registration,
    )?;
    encoder.u64(plan.safepoint().get())?;
    encoder.u32(plan.role().tag())?;
    encoder.u32(plan.root_pair_count())?;
    encoder.fixed(plan.owner().as_array())?;
    encoder.fixed(normalized_stackmap)
}

#[derive(Clone, Copy)]
struct StrongRegistrationDigestInput<'a> {
    kind: DigestKind,
    node: DigestNodeId,
    digest: &'a [u8; 32],
}

impl RuntimeEncode for StrongRegistrationDigestInput<'_> {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.u32(self.kind.tag())?;
        encoder.fixed(self.node.as_array())?;
        encoder.fixed(self.digest)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafepointFingerprintKindV1 {
    ObjectDefinitionNode,
    ObjectDefinition,
    RegistrationDefinition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongSafepointFingerprintError {
    ObjectValidation(StrongSafepointRegistrationValidationError),
    ProofCoverageMismatch,
    MissingObject(SlibMemberId),
    RecordRange(PersistentSafepointSiteId),
    Hash {
        site: PersistentSafepointSiteId,
        kind: SafepointFingerprintKindV1,
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
            Self::ObjectValidation(source) => Some(source),
            Self::Hash { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
