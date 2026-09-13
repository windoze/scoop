use std::fmt;

use scoop_identity::{DigestKind, DigestNodeId, DigestNodeKey, PersistentSafepointSiteId};
use scoop_wire::{
    Encoder, HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, WireEncode,
    domain_separated_runtime_hash,
};

use super::physical::validate_objects;
use super::record::DESCRIPTOR_SIZE;
use super::{StrongSafepointRegistrationValidationError, VerifiedStrongSafepointRegistrationSetV1};
use crate::SlibMemberId;
use crate::link_object::{ScoopLirObjectCandidateV1, StackmapRecordFingerprintV1};

const OBJECT_DEFINITION_DOMAIN: &str = "scoop-object-definition-v1";
const STRONG_REGISTRATION_DOMAIN: &str = "scoop-strong-registration-v1";
const PRIMARY_ATOM_ROLE: u32 = 1;
const SAFEPOINT_REGISTRATION_RECORD_KIND: u32 = 5;
const STRONG_LINKAGE: u32 = 1;

macro_rules! typed_fingerprint {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn as_array(&self) -> &[u8; 32] {
                &self.0
            }
        }

        impl WireEncode for $name {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.bytes(&self.0)
            }
        }

        impl RuntimeEncode for $name {
            fn runtime_encode(
                &self,
                encoder: &mut RuntimeEncoder,
            ) -> Result<(), RuntimeEncodeError> {
                encoder.fixed(&self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                for byte in self.0 {
                    write!(formatter, "{byte:02x}")?;
                }
                Ok(())
            }
        }
    };
}

typed_fingerprint!(ObjectDefinitionFingerprintV1);
typed_fingerprint!(StrongRegistrationFingerprintV1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongSafepointFingerprintV1 {
    site: PersistentSafepointSiteId,
    object_definition_node: DigestNodeId,
    object_definition: ObjectDefinitionFingerprintV1,
    stackmap_node: DigestNodeId,
    stackmap: StackmapRecordFingerprintV1,
    registration_node: DigestNodeId,
    registration: StrongRegistrationFingerprintV1,
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

    pub const fn registration(self) -> StrongRegistrationFingerprintV1 {
        self.registration
    }
}

/// Canonical object and strong-registration fingerprints derived only from a
/// complete safepoint registration proof and the exact verified object bytes.
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
        let record = registration_record_bytes(object, verified.checked_offset(), plan.site())?;
        let object_definition =
            relocation_free_object_definition_fingerprint(record).map_err(|source| {
                StrongSafepointFingerprintError::Hash {
                    site: plan.site(),
                    kind: SafepointFingerprintKindV1::ObjectDefinition,
                    source,
                }
            })?;
        let object_definition_node =
            DigestNodeId::from_key(&DigestNodeKey::object_definition(plan.primary_atom()))
                .map_err(|source| StrongSafepointFingerprintError::Hash {
                    site: plan.site(),
                    kind: SafepointFingerprintKindV1::ObjectDefinitionNode,
                    source,
                })?;
        let stackmap_fingerprint = stackmap.normalized().fingerprint();
        let registration = strong_registration_fingerprint(
            *plan,
            object_definition_node,
            object_definition.as_array(),
            stackmap_fingerprint.as_array(),
        )
        .map_err(|source| StrongSafepointFingerprintError::Hash {
            site: plan.site(),
            kind: SafepointFingerprintKindV1::StrongRegistration,
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
) -> Result<ObjectDefinitionFingerprintV1, HashError> {
    domain_separated_runtime_hash(
        OBJECT_DEFINITION_DOMAIN,
        &RelocationFreeObjectDefinitionInput { bytes },
    )
    .map(|digest| ObjectDefinitionFingerprintV1(*digest.as_array()))
}

struct RelocationFreeObjectDefinitionInput<'a> {
    bytes: &'a [u8],
}

impl RuntimeEncode for RelocationFreeObjectDefinitionInput<'_> {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.u32(PRIMARY_ATOM_ROLE)?;
        encoder.byte_span(self.bytes)?;
        encoder.sequence_length(0)?;
        encoder.sequence_length(0)
    }
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
        .map(|digest| StrongRegistrationFingerprintV1(*digest.as_array()))
}

struct StrongSafepointRegistrationFingerprintInput<'a> {
    plan: scoop_lir::StrongSafepointRegistrationPlanV1,
    direct_inputs: [StrongRegistrationDigestInput<'a>; 2],
    normalized_stackmap: &'a [u8; 32],
}

impl RuntimeEncode for StrongSafepointRegistrationFingerprintInput<'_> {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.u32(SAFEPOINT_REGISTRATION_RECORD_KIND)?;
        encoder.u32(STRONG_LINKAGE)?;
        encoder.fixed(self.plan.site().as_array())?;
        encoder.fixed(&[0; 32])?;
        encoder.fixed(&[0; 32])?;
        encoder.fixed(&[0; 32])?;
        encoder.u64(self.plan.safepoint().get())?;
        encoder.u32(self.plan.role().tag())?;
        encoder.u32(self.plan.root_pair_count())?;
        encoder.fixed(self.plan.owner().as_array())?;
        encoder.fixed(self.normalized_stackmap)?;
        encoder.sequence_length(self.direct_inputs.len())?;
        for input in self.direct_inputs {
            input.runtime_encode(encoder)?;
        }
        Ok(())
    }
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
    StrongRegistration,
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
