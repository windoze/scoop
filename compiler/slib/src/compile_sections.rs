//! Shared envelope decoding for profile-specific Compile section readers.

use std::fmt;

use scoop_identity::{ArtifactCapabilityProfileId, CapabilityId};
use scoop_wire::{WireDecode, WireError};

use crate::{
    ArtifactCapabilityProfile, ArtifactProfileInventoryError, DecodedMetadataEnvelope,
    DecodedMetadataSection, MetadataLocation, MetadataReadError, SlibMemberId, SlibMemberRecord,
    SlibMemberRole, ValidatedGraphArtifact,
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
    ) -> Result<&DecodedMetadataSection<'input>, CompileSectionDecodeError> {
        let envelope = match location {
            MetadataLocation::Hir => &self.hir,
            MetadataLocation::Mir => &self.mir,
            MetadataLocation::Lir => &self.lir,
        };
        envelope
            .sections()
            .iter()
            .find(|section| section.capability() == capability)
            .ok_or_else(|| CompileSectionDecodeError::MissingSection {
                location,
                capability: capability.clone(),
            })
    }
}

pub(crate) fn decode_compile_metadata_envelopes<'input>(
    graph: &'input mut ValidatedGraphArtifact<'_>,
    profile: ArtifactCapabilityProfile,
) -> Result<DecodedCompileMetadataEnvelopes<'input>, CompileSectionDecodeError> {
    validate_compile_profile(graph, profile)?;
    Ok(DecodedCompileMetadataEnvelopes {
        hir: decode_compile_stage(graph, profile, MetadataLocation::Hir)?,
        mir: decode_compile_stage(graph, profile, MetadataLocation::Mir)?,
        lir: decode_compile_stage(graph, profile, MetadataLocation::Lir)?,
    })
}

pub(crate) fn validate_compile_profile(
    graph: &ValidatedGraphArtifact<'_>,
    profile: ArtifactCapabilityProfile,
) -> Result<(), CompileSectionDecodeError> {
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

    Ok(())
}

pub(crate) fn decode_compile_stage<'input>(
    graph: &'input ValidatedGraphArtifact<'_>,
    profile: ArtifactCapabilityProfile,
    location: MetadataLocation,
) -> Result<DecodedMetadataEnvelope<'input>, CompileSectionDecodeError> {
    let member = metadata_member_id(graph, location)?;
    let payload = member_payload(graph, location, member)?;
    let envelope = decode_metadata_envelope(location, payload)?;
    profile
        .validate_compile_metadata_inventory(location, envelope.sections())
        .map_err(CompileSectionDecodeError::Inventory)?;
    Ok(envelope)
}

pub(crate) fn decode_stage_section<T: WireDecode>(
    envelope: &DecodedMetadataEnvelope<'_>,
    location: MetadataLocation,
    capability: CapabilityId,
) -> Result<T, CompileSectionDecodeError> {
    let payload = envelope
        .sections()
        .iter()
        .find(|section| section.capability() == &capability)
        .ok_or_else(|| CompileSectionDecodeError::MissingSection {
            location,
            capability: capability.clone(),
        })?;
    payload
        .decode()
        .map_err(|source| CompileSectionDecodeError::InnerSection {
            location,
            capability,
            source,
        })
}

pub(crate) fn decode_compile_section<T: WireDecode>(
    metadata: &DecodedCompileMetadataEnvelopes<'_>,
    location: MetadataLocation,
    capability: CapabilityId,
) -> Result<T, CompileSectionDecodeError> {
    let payload = metadata.required_section(location, &capability)?;

    payload
        .decode()
        .map_err(|source| CompileSectionDecodeError::InnerSection {
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
    graph: &'input ValidatedGraphArtifact<'_>,
    location: MetadataLocation,
    member: SlibMemberId,
) -> Result<&'input [u8], CompileSectionDecodeError> {
    graph
        .envelope
        .member(member)
        .map_err(CompileSectionDecodeError::MemberRead)?
        .ok_or(CompileSectionDecodeError::MissingMetadataMemberPayload { location, member })
}

#[derive(Debug)]
pub enum CompileSectionDecodeError {
    MemberRead(crate::SlibReadError),
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
}

impl fmt::Display for CompileSectionDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot decode Compile sections: {self:?}")
    }
}

impl std::error::Error for CompileSectionDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MemberRead(error) => Some(error),
            Self::Inventory(error) => Some(error),
            Self::OuterEnvelope { source, .. } => Some(source),
            Self::InnerSection { source, .. } => Some(source),
            Self::WrongProfile { .. }
            | Self::MissingMetadataMember { .. }
            | Self::MissingMetadataMemberPayload { .. }
            | Self::MissingSection { .. } => None,
        }
    }
}
