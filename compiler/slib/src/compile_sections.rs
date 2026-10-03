//! Shared fail-closed front end for profile-specific Compile section readers.

use std::fmt;

use scoop_identity::{ArtifactCapabilityProfileId, CapabilityId};
use scoop_wire::{WireDecode, WireError, decode_canonical};

use crate::{
    ArtifactCapabilityProfile, ArtifactProfileInventoryError, DecodedMetadataEnvelope,
    DecodedMetadataSection, MetadataLocation, MetadataReadError, SemanticFingerprintError,
    SemanticFingerprintRecord, SlibMemberId, SlibMemberRecord, SlibMemberRole,
    ValidatedGraphArtifact,
};

/// Canonically decoded HIR, MIR, and LIR metadata envelopes for one exact
/// Compile profile. Inner capability payloads remain profile-specific and
/// must be decoded before this state can become a semantic proof.
pub(crate) struct DecodedCompileMetadataEnvelopes<'input> {
    hir: DecodedMetadataEnvelope<'input>,
    mir: DecodedMetadataEnvelope<'input>,
    lir: DecodedMetadataEnvelope<'input>,
}

impl<'input> DecodedCompileMetadataEnvelopes<'input> {
    pub(crate) fn required_section(
        &self,
        location: MetadataLocation,
        capability: &CapabilityId,
    ) -> Result<&'input [u8], CompileSectionDecodeError> {
        let envelope = match location {
            MetadataLocation::Hir => &self.hir,
            MetadataLocation::Mir => &self.mir,
            MetadataLocation::Lir => &self.lir,
        };
        envelope
            .sections()
            .iter()
            .find(|section| section.capability() == capability)
            .map(DecodedMetadataSection::payload)
            .ok_or_else(|| CompileSectionDecodeError::MissingSection {
                location,
                capability: capability.clone(),
            })
    }

    pub(crate) fn validate_semantic_fingerprints(
        &self,
        graph: &mut ValidatedGraphArtifact<'_>,
    ) -> Result<(), CompileSectionDecodeError> {
        let actual = {
            let manifest = graph.envelope.manifest();
            SemanticFingerprintRecord::from_decoded_compile_metadata_sections(
                manifest.compatibility(),
                manifest.direct_dependencies(),
                self.hir.sections(),
                self.mir.sections(),
                self.lir.sections(),
            )
        }
        .map_err(CompileSectionDecodeError::SemanticFingerprints)?;
        let expected = graph.envelope.manifest().semantic_fingerprints();
        require_fingerprint(
            MetadataLocation::Hir,
            expected.hir().as_array(),
            actual.hir().as_array(),
        )?;
        require_fingerprint(
            MetadataLocation::Mir,
            expected.mir().as_array(),
            actual.mir().as_array(),
        )?;
        require_fingerprint(
            MetadataLocation::Lir,
            expected.lir().as_array(),
            actual.lir().as_array(),
        )
    }
}

pub(crate) fn decode_compile_metadata_envelopes<'input>(
    graph: &mut ValidatedGraphArtifact<'input>,
    profile: ArtifactCapabilityProfile,
) -> Result<DecodedCompileMetadataEnvelopes<'input>, CompileSectionDecodeError> {
    let expected = profile.id();
    let actual = graph.envelope.manifest().compatibility().artifact_profile();
    if actual != &expected {
        return Err(CompileSectionDecodeError::WrongProfile {
            expected,
            actual: actual.clone(),
        });
    }
    profile
        .validate_compile_manifest_inventory(graph.envelope.manifest().sections())
        .map_err(CompileSectionDecodeError::Inventory)?;

    let hir_member = metadata_member_id(graph, MetadataLocation::Hir)?;
    let mir_member = metadata_member_id(graph, MetadataLocation::Mir)?;
    let lir_member = metadata_member_id(graph, MetadataLocation::Lir)?;
    let hir_payload = member_payload(graph, MetadataLocation::Hir, hir_member)?;
    let mir_payload = member_payload(graph, MetadataLocation::Mir, mir_member)?;
    let lir_payload = member_payload(graph, MetadataLocation::Lir, lir_member)?;

    let hir = decode_metadata_envelope(MetadataLocation::Hir, hir_payload)?;
    let mir = decode_metadata_envelope(MetadataLocation::Mir, mir_payload)?;
    let lir = decode_metadata_envelope(MetadataLocation::Lir, lir_payload)?;

    profile
        .validate_compile_metadata_inventory(MetadataLocation::Hir, hir.sections())
        .map_err(CompileSectionDecodeError::Inventory)?;
    profile
        .validate_compile_metadata_inventory(MetadataLocation::Mir, mir.sections())
        .map_err(CompileSectionDecodeError::Inventory)?;
    profile
        .validate_compile_metadata_inventory(MetadataLocation::Lir, lir.sections())
        .map_err(CompileSectionDecodeError::Inventory)?;

    Ok(DecodedCompileMetadataEnvelopes { hir, mir, lir })
}

pub(crate) fn decode_compile_section<T: WireDecode>(
    metadata: &DecodedCompileMetadataEnvelopes<'_>,
    location: MetadataLocation,
    capability: CapabilityId,
) -> Result<T, CompileSectionDecodeError> {
    let payload = metadata.required_section(location, &capability)?;

    decode_canonical(payload).map_err(|source| CompileSectionDecodeError::InnerSection {
        location,
        capability,
        source,
    })
}

fn decode_metadata_envelope<'input>(
    location: MetadataLocation,
    payload: &'input [u8],
) -> Result<DecodedMetadataEnvelope<'input>, CompileSectionDecodeError> {
    DecodedMetadataEnvelope::decode(payload, location)
        .map_err(|source| CompileSectionDecodeError::OuterEnvelope { location, source })
}

fn metadata_member_id(
    graph: &ValidatedGraphArtifact<'_>,
    location: MetadataLocation,
) -> Result<SlibMemberId, CompileSectionDecodeError> {
    graph
        .envelope
        .manifest()
        .members()
        .iter()
        .find(|member| {
            matches!(
                (location, member.role()),
                (MetadataLocation::Hir, SlibMemberRole::HirMetadata)
                    | (MetadataLocation::Mir, SlibMemberRole::MirMetadata)
                    | (MetadataLocation::Lir, SlibMemberRole::LirMetadata)
            )
        })
        .map(SlibMemberRecord::id)
        .ok_or(CompileSectionDecodeError::MissingMetadataMember { location })
}

fn member_payload<'input>(
    graph: &ValidatedGraphArtifact<'input>,
    location: MetadataLocation,
    member: SlibMemberId,
) -> Result<&'input [u8], CompileSectionDecodeError> {
    graph
        .envelope
        .member(member)
        .ok_or(CompileSectionDecodeError::MissingMetadataMemberPayload { location, member })
}

fn require_fingerprint(
    location: MetadataLocation,
    expected: &[u8; 32],
    actual: &[u8; 32],
) -> Result<(), CompileSectionDecodeError> {
    if expected == actual {
        Ok(())
    } else {
        Err(CompileSectionDecodeError::SemanticFingerprintMismatch {
            location,
            expected: *expected,
            actual: *actual,
        })
    }
}

#[derive(Debug)]
pub enum CompileSectionDecodeError {
    WrongProfile {
        expected: ArtifactCapabilityProfileId,
        actual: ArtifactCapabilityProfileId,
    },
    Inventory(ArtifactProfileInventoryError),
    MissingMetadataMember {
        location: MetadataLocation,
    },
    MissingMetadataMemberPayload {
        location: MetadataLocation,
        member: SlibMemberId,
    },
    OuterEnvelope {
        location: MetadataLocation,
        source: MetadataReadError,
    },
    MissingSection {
        location: MetadataLocation,
        capability: CapabilityId,
    },
    InnerSection {
        location: MetadataLocation,
        capability: CapabilityId,
        source: WireError,
    },
    SemanticFingerprints(SemanticFingerprintError),
    SemanticFingerprintMismatch {
        location: MetadataLocation,
        expected: [u8; 32],
        actual: [u8; 32],
    },
}

impl fmt::Display for CompileSectionDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot decode Compile sections: {self:?}")
    }
}

impl std::error::Error for CompileSectionDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Inventory(error) => Some(error),
            Self::OuterEnvelope { source, .. } => Some(source),
            Self::InnerSection { source, .. } => Some(source),
            Self::SemanticFingerprints(error) => Some(error),
            Self::WrongProfile { .. }
            | Self::MissingMetadataMember { .. }
            | Self::MissingMetadataMemberPayload { .. }
            | Self::MissingSection { .. }
            | Self::SemanticFingerprintMismatch { .. } => None,
        }
    }
}
