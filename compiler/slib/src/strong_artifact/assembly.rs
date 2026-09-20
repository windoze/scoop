use scoop_identity::ConeIdentity;
use scoop_wire::{HashError, WireEncode, encode};

use super::StrongArtifactSectionV1;
use crate::{
    MemberPurposeSet, MemberStableKey, MetadataEnvelope, MetadataEnvelopeError, MetadataLocation,
    MetadataSection, MetadataSectionError, SlibMember, SlibMemberId, SlibMemberRecordError,
    SlibMemberRole, VerifiedCodeFingerprintV1, VerifiedCodeFingerprintV2,
    VerifiedCodeLinkObjectMemberV1,
};

pub(super) fn verify_link_objects(
    code: &VerifiedCodeFingerprintV1,
    actual: &[SlibMember],
) -> Result<(), StrongArtifactAssemblyError> {
    verify_link_object_members(code.production().link_objects().members(), actual)
}

pub(super) fn verify_layout_link_objects(
    code: &VerifiedCodeFingerprintV2,
    actual: &[SlibMember],
) -> Result<(), StrongArtifactAssemblyError> {
    verify_link_object_members(code.production().link_objects().members(), actual)
}

fn verify_link_object_members(
    expected: &[VerifiedCodeLinkObjectMemberV1],
    actual: &[SlibMember],
) -> Result<(), StrongArtifactAssemblyError> {
    if actual.len() != expected.len() {
        return Err(StrongArtifactAssemblyError::LinkObjectCount {
            expected: expected.len(),
            actual: actual.len(),
        });
    }
    for (index, (member, expected)) in actual.iter().zip(expected).enumerate() {
        if !matches!(member.record().role(), SlibMemberRole::LinkObject { .. }) {
            return Err(StrongArtifactAssemblyError::InvalidLinkObjectRole {
                index,
                member: member.record().id(),
            });
        }
        let fingerprint = member
            .record()
            .as_link_member()
            .expect("a LinkObject record has a Link member projection")
            .fingerprint()
            .map_err(StrongArtifactAssemblyError::LinkObjectFingerprint)?;
        if member.record().id() != expected.member() || fingerprint != expected.fingerprint() {
            return Err(StrongArtifactAssemblyError::LinkObjectMismatch {
                index,
                expected: expected.member(),
                actual: member.record().id(),
            });
        }
    }
    Ok(())
}

pub(super) struct LayerAssembly {
    location: MetadataLocation,
    pub(super) sections: Vec<MetadataSection>,
    envelope: Vec<u8>,
}

impl LayerAssembly {
    pub(super) fn new(
        location: MetadataLocation,
        sections: Vec<MetadataSection>,
    ) -> Result<Self, StrongArtifactAssemblyError> {
        let envelope = MetadataEnvelope::new(location, sections.clone())
            .map_err(|source| StrongArtifactAssemblyError::MetadataEnvelope { location, source })?;
        let envelope = encode(&envelope).map_err(|source| {
            StrongArtifactAssemblyError::MetadataEnvelopeEncoding { location, source }
        })?;
        Ok(Self {
            location,
            sections,
            envelope,
        })
    }

    pub(super) fn into_member(
        self,
        cone: ConeIdentity,
    ) -> Result<SlibMember, StrongArtifactAssemblyError> {
        let (stable_key, role) = match self.location {
            MetadataLocation::Hir => (MemberStableKey::HirMetadata, SlibMemberRole::HirMetadata),
            MetadataLocation::Mir => (MemberStableKey::MirMetadata, SlibMemberRole::MirMetadata),
            MetadataLocation::Lir => (MemberStableKey::LirMetadata, SlibMemberRole::LirMetadata),
        };
        SlibMember::new(cone, stable_key, role, self.envelope).map_err(|source| {
            StrongArtifactAssemblyError::MetadataMember {
                location: self.location,
                source,
            }
        })
    }
}

pub(super) fn build_section(
    section: StrongArtifactSectionV1,
    location: MetadataLocation,
    capability: scoop_identity::CapabilityId,
    required_for: MemberPurposeSet,
    value: &impl WireEncode,
) -> Result<MetadataSection, StrongArtifactAssemblyError> {
    let payload = encode(value)
        .map_err(|source| StrongArtifactAssemblyError::Encoding { section, source })?;
    MetadataSection::new(location, capability, required_for, payload)
        .map_err(|source| StrongArtifactAssemblyError::MetadataSection { section, source })
}

#[derive(Debug)]
pub(super) enum StrongArtifactAssemblyError {
    LinkObjectCount {
        expected: usize,
        actual: usize,
    },
    InvalidLinkObjectRole {
        index: usize,
        member: SlibMemberId,
    },
    LinkObjectMismatch {
        index: usize,
        expected: SlibMemberId,
        actual: SlibMemberId,
    },
    LinkObjectFingerprint(HashError),
    Encoding {
        section: StrongArtifactSectionV1,
        source: scoop_wire::cbor::EncodeError,
    },
    MetadataSection {
        section: StrongArtifactSectionV1,
        source: MetadataSectionError,
    },
    MetadataEnvelope {
        location: MetadataLocation,
        source: MetadataEnvelopeError,
    },
    MetadataEnvelopeEncoding {
        location: MetadataLocation,
        source: scoop_wire::cbor::EncodeError,
    },
    MetadataMember {
        location: MetadataLocation,
        source: SlibMemberRecordError,
    },
}
