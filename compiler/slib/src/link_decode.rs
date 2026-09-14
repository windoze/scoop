//! Link-view section inventory and atomic payload decoding.

use std::fmt;

use scoop_identity::{ArtifactCapabilityProfileId, CapabilityId, ConeCoordinate, ConeIdentity};
use scoop_lir::DecodedStrongProductionSectionV1;
use scoop_wire::{DecodeUsage, WireError, decode_canonical_with_meter};

use crate::{
    ArtifactCapabilityProfile, ArtifactFingerprint, ArtifactProfileLinkInventoryError,
    DecodedLinkIdentityClosureSectionV1, DecodedMetadataEnvelope,
    DecodedSingleConeProductionManifestV1, ManifestSection, MetadataLocation, MetadataReadError,
    SlibMemberId, SlibMemberRecord, SlibMemberRole, ValidatedGraphArtifact,
    lir_link_identity_closure_capability, lir_strong_production_capability,
    manifest_single_cone_production_capability,
};

const LINK_SECTION_HANDLER_BASE_WORK: u64 = 64;

/// The three Link payloads decoded atomically from one strong-profile graph
/// artifact. No identity, fingerprint, object, or manifest value has yet been
/// promoted to a Link proof.
#[derive(Debug)]
pub struct DecodedSingleConeLinkSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    strong_production: DecodedStrongProductionSectionV1,
    link_identity_closure: DecodedLinkIdentityClosureSectionV1,
    production_manifest: DecodedSingleConeProductionManifestV1,
}

impl<'input> ValidatedGraphArtifact<'input> {
    pub fn decode_single_cone_link_sections(
        mut self,
    ) -> Result<DecodedSingleConeLinkSections<'input>, SingleConeLinkSectionDecodeError> {
        let profile = require_strong_profile(&self)?;
        let production_manifest = decode_production_manifest(&mut self, profile)?;
        let lir_member_id = metadata_member_id(&self, MetadataLocation::Lir)?;
        let (lir_payload, meter) = self.envelope.member_and_meter(lir_member_id);
        let lir_payload = lir_payload.ok_or(
            SingleConeLinkSectionDecodeError::MissingMetadataMemberPayload {
                location: MetadataLocation::Lir,
                member: lir_member_id,
            },
        )?;
        let lir_envelope =
            DecodedMetadataEnvelope::decode_with_meter(lir_payload, MetadataLocation::Lir, meter)
                .map_err(SingleConeLinkSectionDecodeError::LirEnvelope)?;
        profile
            .validate_link_metadata_inventory(MetadataLocation::Lir, lir_envelope.sections())
            .map_err(SingleConeLinkSectionDecodeError::Inventory)?;

        let strong_payload =
            required_lir_section(&lir_envelope, lir_strong_production_capability())?;
        let closure_payload =
            required_lir_section(&lir_envelope, lir_link_identity_closure_capability())?;
        self.envelope
            .meter_mut()
            .charge_work(LINK_SECTION_HANDLER_BASE_WORK, &Default::default())
            .map_err(SingleConeLinkSectionDecodeError::Resource)?;
        let strong_production = decode_canonical_with_meter::<DecodedStrongProductionSectionV1>(
            strong_payload,
            self.envelope.meter_mut(),
        )
        .map_err(|source| SingleConeLinkSectionDecodeError::InnerSection {
            capability: lir_strong_production_capability(),
            source,
        })?;
        let link_identity_closure = decode_canonical_with_meter::<
            DecodedLinkIdentityClosureSectionV1,
        >(closure_payload, self.envelope.meter_mut())
        .map_err(|source| SingleConeLinkSectionDecodeError::InnerSection {
            capability: lir_link_identity_closure_capability(),
            source,
        })?;

        Ok(DecodedSingleConeLinkSections {
            graph: self,
            strong_production,
            link_identity_closure,
            production_manifest,
        })
    }
}

impl DecodedSingleConeLinkSections<'_> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.graph.artifact_fingerprint()
    }

    pub const fn decode_usage(&self) -> DecodeUsage {
        self.graph.decode_usage()
    }

    pub const fn strong_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        &self.strong_production
    }

    pub const fn link_identity_closure_wire(&self) -> &DecodedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn production_manifest_wire(&self) -> &DecodedSingleConeProductionManifestV1 {
        &self.production_manifest
    }
}

fn require_strong_profile(
    graph: &ValidatedGraphArtifact<'_>,
) -> Result<ArtifactCapabilityProfile, SingleConeLinkSectionDecodeError> {
    let actual = graph.envelope.manifest().compatibility().artifact_profile();
    if actual != &ArtifactCapabilityProfileId::single_cone_strong() {
        return Err(SingleConeLinkSectionDecodeError::WrongProfile {
            actual: actual.clone(),
        });
    }
    Ok(ArtifactCapabilityProfile::SINGLE_CONE_STRONG)
}

fn decode_production_manifest(
    graph: &mut ValidatedGraphArtifact<'_>,
    profile: ArtifactCapabilityProfile,
) -> Result<DecodedSingleConeProductionManifestV1, SingleConeLinkSectionDecodeError> {
    let (manifest, meter) = graph.envelope.manifest_and_meter();
    profile
        .validate_link_manifest_inventory(manifest.sections())
        .map_err(SingleConeLinkSectionDecodeError::Inventory)?;
    let section = required_manifest_section(
        manifest.sections(),
        manifest_single_cone_production_capability(),
    )?;
    meter
        .charge_work(LINK_SECTION_HANDLER_BASE_WORK, &Default::default())
        .map_err(SingleConeLinkSectionDecodeError::Resource)?;
    decode_canonical_with_meter::<DecodedSingleConeProductionManifestV1>(section.payload(), meter)
        .map_err(|source| SingleConeLinkSectionDecodeError::InnerSection {
            capability: manifest_single_cone_production_capability(),
            source,
        })
}

fn metadata_member_id(
    graph: &ValidatedGraphArtifact<'_>,
    location: MetadataLocation,
) -> Result<SlibMemberId, SingleConeLinkSectionDecodeError> {
    graph
        .envelope
        .manifest()
        .members()
        .iter()
        .find(|member| {
            matches!(
                (location, member.role()),
                (MetadataLocation::Lir, SlibMemberRole::LirMetadata)
            )
        })
        .map(SlibMemberRecord::id)
        .ok_or(SingleConeLinkSectionDecodeError::MissingMetadataMember { location })
}

fn required_manifest_section(
    sections: &[ManifestSection],
    capability: CapabilityId,
) -> Result<&ManifestSection, SingleConeLinkSectionDecodeError> {
    sections
        .iter()
        .find(|section| section.capability() == &capability)
        .ok_or(SingleConeLinkSectionDecodeError::MissingSection { capability })
}

fn required_lir_section<'input>(
    envelope: &DecodedMetadataEnvelope<'input>,
    capability: CapabilityId,
) -> Result<&'input [u8], SingleConeLinkSectionDecodeError> {
    envelope
        .sections()
        .iter()
        .find(|section| section.capability() == &capability)
        .map(crate::DecodedMetadataSection::payload)
        .ok_or(SingleConeLinkSectionDecodeError::MissingSection { capability })
}

#[derive(Debug)]
pub enum SingleConeLinkSectionDecodeError {
    WrongProfile {
        actual: ArtifactCapabilityProfileId,
    },
    Inventory(ArtifactProfileLinkInventoryError),
    MissingMetadataMember {
        location: MetadataLocation,
    },
    MissingMetadataMemberPayload {
        location: MetadataLocation,
        member: SlibMemberId,
    },
    LirEnvelope(MetadataReadError),
    MissingSection {
        capability: CapabilityId,
    },
    InnerSection {
        capability: CapabilityId,
        source: WireError,
    },
    Resource(WireError),
}

impl fmt::Display for SingleConeLinkSectionDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot decode single-Cone Link sections: {self:?}"
        )
    }
}

impl std::error::Error for SingleConeLinkSectionDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Inventory(error) => Some(error),
            Self::LirEnvelope(error) => Some(error),
            Self::InnerSection { source, .. } | Self::Resource(source) => Some(source),
            Self::WrongProfile { .. }
            | Self::MissingMetadataMember { .. }
            | Self::MissingMetadataMemberPayload { .. }
            | Self::MissingSection { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests;
