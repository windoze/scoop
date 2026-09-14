//! Link-view section inventory and atomic payload decoding.

use std::fmt;

use scoop_hir::{DecodedHirFoundation, OdrFreeHirFoundation};
use scoop_identity::{
    ArtifactCapabilityProfileId, CapabilityId, ConeCoordinate, ConeIdentity,
    IdentityValidationError, ValidatedIdentityGraph,
};
use scoop_lir::{DecodedLirFoundation, DecodedStrongProductionSectionV1, OdrFreeLirFoundation};
use scoop_mir::{DecodedMirFoundation, OdrFreeMirFoundation};
use scoop_wire::{DecodeUsage, WireDecode, WireError, WirePath, decode_canonical_with_meter};

use crate::compile_decode::validate_foundation_identity_graph;
use crate::strong_compile_decode::{
    StrongProfileFoundationError, validate_strong_profile_foundations,
};
use crate::{
    ArtifactCapabilityProfile, ArtifactFingerprint, ArtifactProfileInventoryError,
    DecodedLinkIdentityClosureSectionV1, DecodedMetadataEnvelope,
    DecodedSingleConeProductionManifestV1, ManifestSection, MetadataLocation, MetadataReadError,
    SemanticFingerprintError, SemanticFingerprintRecord, SlibMemberId, SlibMemberRecord,
    SlibMemberRole, ValidatedGraphArtifact, hir_identity_foundation_capability,
    lir_identity_foundation_capability, lir_link_identity_closure_capability,
    lir_strong_production_capability, manifest_single_cone_production_capability,
    mir_identity_foundation_capability,
};

const LINK_SECTION_HANDLER_BASE_WORK: u64 = 64;

/// The three Link payloads decoded atomically from one strong-profile graph
/// artifact. No identity, fingerprint, object, or manifest value has yet been
/// promoted to a Link proof.
#[derive(Debug)]
pub struct DecodedSingleConeLinkSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    hir_foundation: DecodedHirFoundation,
    mir_foundation: DecodedMirFoundation,
    lir_foundation: DecodedLirFoundation,
    strong_production: DecodedStrongProductionSectionV1,
    link_identity_closure: DecodedLinkIdentityClosureSectionV1,
    production_manifest: DecodedSingleConeProductionManifestV1,
}

/// Link sections whose complete HIR-to-LIR foundation identity graph passed
/// one transaction. Foundation structure and the strong profile's ODR policy
/// remain unproven.
pub struct IdentityCheckedSingleConeLinkSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    hir_foundation: DecodedHirFoundation,
    mir_foundation: DecodedMirFoundation,
    lir_foundation: DecodedLirFoundation,
    strong_production: DecodedStrongProductionSectionV1,
    link_identity_closure: DecodedLinkIdentityClosureSectionV1,
    production_manifest: DecodedSingleConeProductionManifestV1,
}

/// Link sections backed by structurally valid foundations that satisfy the
/// `SingleConeStrongProfile` `RejectAll` ODR policy.
pub struct OdrCheckedSingleConeLinkFoundations<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    hir_foundation: OdrFreeHirFoundation,
    mir_foundation: OdrFreeMirFoundation,
    lir_foundation: OdrFreeLirFoundation,
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

        let hir_member_id = metadata_member_id(&self, MetadataLocation::Hir)?;
        let mir_member_id = metadata_member_id(&self, MetadataLocation::Mir)?;
        let lir_member_id = metadata_member_id(&self, MetadataLocation::Lir)?;
        let hir_payload = member_payload(&self, MetadataLocation::Hir, hir_member_id)?;
        let mir_payload = member_payload(&self, MetadataLocation::Mir, mir_member_id)?;
        let lir_payload = member_payload(&self, MetadataLocation::Lir, lir_member_id)?;

        let hir_envelope = decode_metadata_envelope(&mut self, hir_payload, MetadataLocation::Hir)?;
        let mir_envelope = decode_metadata_envelope(&mut self, mir_payload, MetadataLocation::Mir)?;
        let lir_envelope = decode_metadata_envelope(&mut self, lir_payload, MetadataLocation::Lir)?;
        profile
            .validate_link_metadata_inventory(MetadataLocation::Hir, hir_envelope.sections())
            .map_err(SingleConeLinkSectionDecodeError::Inventory)?;
        profile
            .validate_link_metadata_inventory(MetadataLocation::Mir, mir_envelope.sections())
            .map_err(SingleConeLinkSectionDecodeError::Inventory)?;
        profile
            .validate_link_metadata_inventory(MetadataLocation::Lir, lir_envelope.sections())
            .map_err(SingleConeLinkSectionDecodeError::Inventory)?;

        let hir_foundation_capability = hir_identity_foundation_capability();
        let mir_foundation_capability = mir_identity_foundation_capability();
        let lir_foundation_capability = lir_identity_foundation_capability();
        let strong_production_capability = lir_strong_production_capability();
        let link_identity_closure_capability = lir_link_identity_closure_capability();
        let hir_foundation_payload =
            required_metadata_section(&hir_envelope, &hir_foundation_capability)?;
        let mir_foundation_payload =
            required_metadata_section(&mir_envelope, &mir_foundation_capability)?;
        let lir_foundation_payload =
            required_metadata_section(&lir_envelope, &lir_foundation_capability)?;
        let strong_payload =
            required_metadata_section(&lir_envelope, &strong_production_capability)?;
        let closure_payload =
            required_metadata_section(&lir_envelope, &link_identity_closure_capability)?;

        let hir_foundation = decode_inner(
            &mut self,
            MetadataLocation::Hir,
            hir_foundation_capability,
            hir_foundation_payload,
        )?;
        let mir_foundation = decode_inner(
            &mut self,
            MetadataLocation::Mir,
            mir_foundation_capability,
            mir_foundation_payload,
        )?;
        let lir_foundation = decode_inner(
            &mut self,
            MetadataLocation::Lir,
            lir_foundation_capability,
            lir_foundation_payload,
        )?;
        let strong_production = decode_inner(
            &mut self,
            MetadataLocation::Lir,
            strong_production_capability,
            strong_payload,
        )?;
        let link_identity_closure = decode_inner(
            &mut self,
            MetadataLocation::Lir,
            link_identity_closure_capability,
            closure_payload,
        )?;
        validate_semantic_fingerprints(&mut self, &hir_envelope, &mir_envelope, &lir_envelope)?;

        Ok(DecodedSingleConeLinkSections {
            graph: self,
            hir_foundation,
            mir_foundation,
            lir_foundation,
            strong_production,
            link_identity_closure,
            production_manifest,
        })
    }
}

impl<'input> DecodedSingleConeLinkSections<'input> {
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

    pub const fn hir_foundation_wire(&self) -> &DecodedHirFoundation {
        &self.hir_foundation
    }

    pub const fn mir_foundation_wire(&self) -> &DecodedMirFoundation {
        &self.mir_foundation
    }

    pub const fn lir_foundation_wire(&self) -> &DecodedLirFoundation {
        &self.lir_foundation
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

    /// Registers and resolves every foundation identity before any Link
    /// payload can use a persistent identity as trusted input.
    pub fn validate_identities(
        self,
    ) -> Result<IdentityCheckedSingleConeLinkSections<'input>, IdentityValidationError> {
        let Self {
            mut graph,
            hir_foundation,
            mir_foundation,
            lir_foundation,
            strong_production,
            link_identity_closure,
            production_manifest,
        } = self;
        let identities = validate_foundation_identity_graph(
            &mut graph,
            &hir_foundation,
            &mir_foundation,
            &lir_foundation,
        )?;
        Ok(IdentityCheckedSingleConeLinkSections {
            graph,
            identities,
            hir_foundation,
            mir_foundation,
            lir_foundation,
            strong_production,
            link_identity_closure,
            production_manifest,
        })
    }
}

impl<'input> IdentityCheckedSingleConeLinkSections<'input> {
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

    pub fn identity_count(&self) -> usize {
        self.identities.identity_count()
    }

    pub fn declared_identity_count(&self) -> usize {
        self.identities.declared_identity_count()
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

    pub fn validate_foundation_structure(
        self,
    ) -> Result<OdrCheckedSingleConeLinkFoundations<'input>, StrongProfileFoundationError> {
        let Self {
            mut graph,
            mut identities,
            hir_foundation,
            mir_foundation,
            lir_foundation,
            strong_production,
            link_identity_closure,
            production_manifest,
        } = self;
        let foundations = validate_strong_profile_foundations(
            &mut graph,
            &mut identities,
            hir_foundation,
            mir_foundation,
            lir_foundation,
        )?;
        Ok(OdrCheckedSingleConeLinkFoundations {
            graph,
            identities,
            hir_foundation: foundations.hir,
            mir_foundation: foundations.mir,
            lir_foundation: foundations.lir,
            strong_production,
            link_identity_closure,
            production_manifest,
        })
    }
}

impl OdrCheckedSingleConeLinkFoundations<'_> {
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

    pub fn identity_count(&self) -> usize {
        self.identities.identity_count()
    }

    pub fn declared_identity_count(&self) -> usize {
        self.identities.declared_identity_count()
    }

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.hir_foundation
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.mir_foundation
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.lir_foundation
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
            location: None,
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
                (MetadataLocation::Hir, SlibMemberRole::HirMetadata)
                    | (MetadataLocation::Mir, SlibMemberRole::MirMetadata)
                    | (MetadataLocation::Lir, SlibMemberRole::LirMetadata)
            )
        })
        .map(SlibMemberRecord::id)
        .ok_or(SingleConeLinkSectionDecodeError::MissingMetadataMember { location })
}

fn member_payload<'input>(
    graph: &ValidatedGraphArtifact<'input>,
    location: MetadataLocation,
    member: SlibMemberId,
) -> Result<&'input [u8], SingleConeLinkSectionDecodeError> {
    graph
        .envelope
        .member(member)
        .ok_or(SingleConeLinkSectionDecodeError::MissingMetadataMemberPayload { location, member })
}

fn decode_metadata_envelope<'input>(
    graph: &mut ValidatedGraphArtifact<'input>,
    payload: &'input [u8],
    location: MetadataLocation,
) -> Result<DecodedMetadataEnvelope<'input>, SingleConeLinkSectionDecodeError> {
    DecodedMetadataEnvelope::decode_with_meter(payload, location, graph.envelope.meter_mut())
        .map_err(|source| SingleConeLinkSectionDecodeError::OuterEnvelope { location, source })
}

fn required_manifest_section(
    sections: &[ManifestSection],
    capability: CapabilityId,
) -> Result<&ManifestSection, SingleConeLinkSectionDecodeError> {
    sections
        .iter()
        .find(|section| section.capability() == &capability)
        .ok_or(SingleConeLinkSectionDecodeError::MissingSection {
            location: None,
            capability,
        })
}

fn required_metadata_section<'input>(
    envelope: &DecodedMetadataEnvelope<'input>,
    capability: &CapabilityId,
) -> Result<&'input [u8], SingleConeLinkSectionDecodeError> {
    envelope
        .sections()
        .iter()
        .find(|section| section.capability() == capability)
        .map(crate::DecodedMetadataSection::payload)
        .ok_or_else(|| SingleConeLinkSectionDecodeError::MissingSection {
            location: Some(envelope.location()),
            capability: capability.clone(),
        })
}

fn decode_inner<T: WireDecode>(
    graph: &mut ValidatedGraphArtifact<'_>,
    location: MetadataLocation,
    capability: CapabilityId,
    payload: &[u8],
) -> Result<T, SingleConeLinkSectionDecodeError> {
    let meter = graph.envelope.meter_mut();
    meter
        .charge_work(LINK_SECTION_HANDLER_BASE_WORK, &WirePath::root())
        .map_err(|source| SingleConeLinkSectionDecodeError::InnerSection {
            location: Some(location),
            capability: capability.clone(),
            source,
        })?;
    decode_canonical_with_meter(payload, meter).map_err(|source| {
        SingleConeLinkSectionDecodeError::InnerSection {
            location: Some(location),
            capability,
            source,
        }
    })
}

fn validate_semantic_fingerprints(
    graph: &mut ValidatedGraphArtifact<'_>,
    hir: &DecodedMetadataEnvelope<'_>,
    mir: &DecodedMetadataEnvelope<'_>,
    lir: &DecodedMetadataEnvelope<'_>,
) -> Result<(), SingleConeLinkSectionDecodeError> {
    let actual = {
        let (manifest, meter) = graph.envelope.manifest_and_meter();
        SemanticFingerprintRecord::from_decoded_compile_metadata_sections(
            manifest.compatibility(),
            manifest.direct_dependencies(),
            hir.sections(),
            mir.sections(),
            lir.sections(),
            meter,
        )
    }
    .map_err(SingleConeLinkSectionDecodeError::SemanticFingerprints)?;
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

fn require_fingerprint(
    location: MetadataLocation,
    expected: &[u8; 32],
    actual: &[u8; 32],
) -> Result<(), SingleConeLinkSectionDecodeError> {
    if expected == actual {
        Ok(())
    } else {
        Err(
            SingleConeLinkSectionDecodeError::SemanticFingerprintMismatch {
                location,
                expected: *expected,
                actual: *actual,
            },
        )
    }
}

#[derive(Debug)]
pub enum SingleConeLinkSectionDecodeError {
    WrongProfile {
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
        location: Option<MetadataLocation>,
        capability: CapabilityId,
    },
    InnerSection {
        location: Option<MetadataLocation>,
        capability: CapabilityId,
        source: WireError,
    },
    Resource(WireError),
    SemanticFingerprints(SemanticFingerprintError),
    SemanticFingerprintMismatch {
        location: MetadataLocation,
        expected: [u8; 32],
        actual: [u8; 32],
    },
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
            Self::OuterEnvelope { source, .. } => Some(source),
            Self::InnerSection { source, .. } | Self::Resource(source) => Some(source),
            Self::SemanticFingerprints(error) => Some(error),
            Self::WrongProfile { .. }
            | Self::MissingMetadataMember { .. }
            | Self::MissingMetadataMemberPayload { .. }
            | Self::MissingSection { .. }
            | Self::SemanticFingerprintMismatch { .. } => None,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;

#[cfg(test)]
pub(crate) fn strong_production_fixture_for_test(
    coordinate: ConeCoordinate,
) -> (
    scoop_lir::CanonicalLirFoundation,
    scoop_lir::StrongProductionSectionV1,
) {
    tests::strong_production_fixture(coordinate)
}
