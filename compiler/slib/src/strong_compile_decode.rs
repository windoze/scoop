//! Compile-view section inventory and atomic payload decoding for the
//! single-Cone strong profile.

use std::fmt;

use scoop_hir::{
    DecodedCoreBootstrapInterfaceSectionV1, DecodedHirFoundation, HirFoundationValidationError,
    OdrFreeHirFoundation, OdrFreeHirFoundationError,
};
use scoop_identity::{
    ArtifactCapabilityProfileId, CapabilityId, ConeCoordinate, ConeIdentity,
    IdentityValidationError, PendingIdentityValidation, ValidatedIdentityGraph,
};
use scoop_lir::{
    DecodedLirFoundation, DecodedStrongProductionSectionV1, LirFoundationValidationError,
    OdrFreeLirFoundation, OdrFreeLirFoundationError,
};
use scoop_mir::{
    DecodedCoreBootstrapBridgeSectionV1, DecodedMirFoundation, MirFoundationValidationError,
    OdrFreeMirFoundation, OdrFreeMirFoundationError,
};
use scoop_wire::{DecodeUsage, WireDecode, WireError, WirePath, decode_canonical_with_meter};

use crate::{
    ArtifactCapabilityProfile, ArtifactFingerprint, ArtifactProfileInventoryError,
    DecodedMetadataEnvelope, DecodedMetadataSection, MetadataLocation, MetadataReadError,
    SemanticFingerprintError, SemanticFingerprintRecord, SlibMemberId, SlibMemberRecord,
    SlibMemberRole, ValidatedGraphArtifact, hir_core_bootstrap_interface_capability,
    hir_identity_foundation_capability, lir_identity_foundation_capability,
    lir_strong_production_capability, mir_core_bootstrap_bridge_capability,
    mir_identity_foundation_capability,
};

const COMPILE_SECTION_HANDLER_BASE_WORK: u64 = 64;

/// Every Compile payload required by one strong-profile graph artifact,
/// decoded as a single transaction. Persistent identities and cross-section
/// structure have not yet been validated or committed to a semantic session.
#[derive(Debug)]
pub struct DecodedSingleConeCompileSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    hir_foundation: DecodedHirFoundation,
    hir_production: DecodedCoreBootstrapInterfaceSectionV1,
    mir_foundation: DecodedMirFoundation,
    mir_production: DecodedCoreBootstrapBridgeSectionV1,
    lir_foundation: DecodedLirFoundation,
    lir_production: DecodedStrongProductionSectionV1,
}

/// Strong-profile Compile sections whose complete HIR-to-LIR identity graph
/// passed one transaction. Foundation structure, ODR policy, and production
/// relations remain unproven.
pub struct IdentityCheckedSingleConeCompileSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    hir_foundation: DecodedHirFoundation,
    hir_production: DecodedCoreBootstrapInterfaceSectionV1,
    mir_foundation: DecodedMirFoundation,
    mir_production: DecodedCoreBootstrapBridgeSectionV1,
    lir_foundation: DecodedLirFoundation,
    lir_production: DecodedStrongProductionSectionV1,
}

/// Strong-profile foundations whose local structure and `RejectAll` ODR
/// policy both passed. Production sections remain decoded references and are
/// validated only by the following cross-section phase.
pub struct OdrCheckedSingleConeCompileFoundations<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    hir_foundation: OdrFreeHirFoundation,
    hir_production: DecodedCoreBootstrapInterfaceSectionV1,
    mir_foundation: OdrFreeMirFoundation,
    mir_production: DecodedCoreBootstrapBridgeSectionV1,
    lir_foundation: OdrFreeLirFoundation,
    lir_production: DecodedStrongProductionSectionV1,
}

impl<'input> ValidatedGraphArtifact<'input> {
    pub fn decode_single_cone_compile_sections(
        mut self,
    ) -> Result<DecodedSingleConeCompileSections<'input>, SingleConeCompileSectionDecodeError> {
        let profile = require_strong_profile(&self)?;
        profile
            .validate_compile_manifest_inventory(self.envelope.manifest().sections())
            .map_err(SingleConeCompileSectionDecodeError::Inventory)?;

        let hir_member = metadata_member_id(&self, MetadataLocation::Hir)?;
        let mir_member = metadata_member_id(&self, MetadataLocation::Mir)?;
        let lir_member = metadata_member_id(&self, MetadataLocation::Lir)?;
        let hir_payload = member_payload(&self, MetadataLocation::Hir, hir_member)?;
        let mir_payload = member_payload(&self, MetadataLocation::Mir, mir_member)?;
        let lir_payload = member_payload(&self, MetadataLocation::Lir, lir_member)?;

        let hir_envelope = DecodedMetadataEnvelope::decode_with_meter(
            hir_payload,
            MetadataLocation::Hir,
            self.envelope.meter_mut(),
        )
        .map_err(
            |source| SingleConeCompileSectionDecodeError::OuterEnvelope {
                location: MetadataLocation::Hir,
                source,
            },
        )?;
        let mir_envelope = DecodedMetadataEnvelope::decode_with_meter(
            mir_payload,
            MetadataLocation::Mir,
            self.envelope.meter_mut(),
        )
        .map_err(
            |source| SingleConeCompileSectionDecodeError::OuterEnvelope {
                location: MetadataLocation::Mir,
                source,
            },
        )?;
        let lir_envelope = DecodedMetadataEnvelope::decode_with_meter(
            lir_payload,
            MetadataLocation::Lir,
            self.envelope.meter_mut(),
        )
        .map_err(
            |source| SingleConeCompileSectionDecodeError::OuterEnvelope {
                location: MetadataLocation::Lir,
                source,
            },
        )?;

        profile
            .validate_compile_metadata_inventory(MetadataLocation::Hir, hir_envelope.sections())
            .map_err(SingleConeCompileSectionDecodeError::Inventory)?;
        profile
            .validate_compile_metadata_inventory(MetadataLocation::Mir, mir_envelope.sections())
            .map_err(SingleConeCompileSectionDecodeError::Inventory)?;
        profile
            .validate_compile_metadata_inventory(MetadataLocation::Lir, lir_envelope.sections())
            .map_err(SingleConeCompileSectionDecodeError::Inventory)?;

        let hir_foundation_capability = hir_identity_foundation_capability();
        let hir_production_capability = hir_core_bootstrap_interface_capability();
        let mir_foundation_capability = mir_identity_foundation_capability();
        let mir_production_capability = mir_core_bootstrap_bridge_capability();
        let lir_foundation_capability = lir_identity_foundation_capability();
        let lir_production_capability = lir_strong_production_capability();
        let hir_foundation_payload = required_section(&hir_envelope, &hir_foundation_capability)?;
        let hir_production_payload = required_section(&hir_envelope, &hir_production_capability)?;
        let mir_foundation_payload = required_section(&mir_envelope, &mir_foundation_capability)?;
        let mir_production_payload = required_section(&mir_envelope, &mir_production_capability)?;
        let lir_foundation_payload = required_section(&lir_envelope, &lir_foundation_capability)?;
        let lir_production_payload = required_section(&lir_envelope, &lir_production_capability)?;

        let hir_foundation = decode_inner(
            &mut self,
            MetadataLocation::Hir,
            hir_foundation_capability,
            hir_foundation_payload,
        )?;
        let hir_production = decode_inner(
            &mut self,
            MetadataLocation::Hir,
            hir_production_capability,
            hir_production_payload,
        )?;
        let mir_foundation = decode_inner(
            &mut self,
            MetadataLocation::Mir,
            mir_foundation_capability,
            mir_foundation_payload,
        )?;
        let mir_production = decode_inner(
            &mut self,
            MetadataLocation::Mir,
            mir_production_capability,
            mir_production_payload,
        )?;
        let lir_foundation = decode_inner(
            &mut self,
            MetadataLocation::Lir,
            lir_foundation_capability,
            lir_foundation_payload,
        )?;
        let lir_production = decode_inner(
            &mut self,
            MetadataLocation::Lir,
            lir_production_capability,
            lir_production_payload,
        )?;

        validate_semantic_fingerprints(&mut self, &hir_envelope, &mir_envelope, &lir_envelope)?;

        Ok(DecodedSingleConeCompileSections {
            graph: self,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            lir_production,
        })
    }
}

impl<'input> DecodedSingleConeCompileSections<'input> {
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

    pub const fn hir_production_wire(&self) -> &DecodedCoreBootstrapInterfaceSectionV1 {
        &self.hir_production
    }

    pub const fn mir_foundation_wire(&self) -> &DecodedMirFoundation {
        &self.mir_foundation
    }

    pub const fn mir_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_production
    }

    pub const fn lir_foundation_wire(&self) -> &DecodedLirFoundation {
        &self.lir_foundation
    }

    pub const fn lir_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        &self.lir_production
    }

    /// Registers and resolves every foundation identity before exposing any
    /// trusted persistent id to production-section validation.
    pub fn validate_identities(
        self,
    ) -> Result<IdentityCheckedSingleConeCompileSections<'input>, IdentityValidationError> {
        let Self {
            mut graph,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            lir_production,
        } = self;
        let producer = graph.identity();
        let (manifest, meter) = graph.envelope.manifest_and_meter();
        let mut validation = PendingIdentityValidation::with_meter(meter);
        validation.register_authority(ConeIdentity::CORE)?;
        if producer != ConeIdentity::CORE {
            validation.register_authority(producer)?;
        }
        for dependency in manifest.direct_dependencies() {
            let authority = dependency.identity();
            if authority != ConeIdentity::CORE && authority != producer {
                validation.register_authority(authority)?;
            }
        }

        hir_foundation.register_identities(&mut validation)?;
        mir_foundation.register_identities(&mut validation)?;
        lir_foundation.register_identities(&mut validation)?;
        hir_foundation.resolve_identities(&mut validation)?;
        mir_foundation.resolve_identities(&mut validation)?;
        lir_foundation.resolve_identities(&mut validation)?;
        let identities = validation.finish()?;

        Ok(IdentityCheckedSingleConeCompileSections {
            graph,
            identities,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            lir_production,
        })
    }
}

impl<'input> IdentityCheckedSingleConeCompileSections<'input> {
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

    pub const fn hir_foundation_wire(&self) -> &DecodedHirFoundation {
        &self.hir_foundation
    }

    pub const fn hir_production_wire(&self) -> &DecodedCoreBootstrapInterfaceSectionV1 {
        &self.hir_production
    }

    pub const fn mir_foundation_wire(&self) -> &DecodedMirFoundation {
        &self.mir_foundation
    }

    pub const fn mir_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_production
    }

    pub const fn lir_foundation_wire(&self) -> &DecodedLirFoundation {
        &self.lir_foundation
    }

    pub const fn lir_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        &self.lir_production
    }

    /// Validates all three foundation structures before replaying the strong
    /// profile's ODR rejection over each canonical layer.
    pub fn validate_foundation_structure(
        self,
    ) -> Result<OdrCheckedSingleConeCompileFoundations<'input>, StrongCompileFoundationError> {
        let Self {
            mut graph,
            mut identities,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            lir_production,
        } = self;
        let coordinate = graph.coordinate().clone();
        let producer = graph.identity();
        let meter = graph.envelope.meter_mut();
        let hir_foundation = hir_foundation
            .validate(&coordinate, &mut identities, meter)
            .map_err(StrongCompileFoundationError::HirStructure)?;
        let mir_foundation = mir_foundation
            .validate(&mut identities, meter)
            .map_err(StrongCompileFoundationError::MirStructure)?;
        let lir_foundation = lir_foundation
            .validate(producer, &mut identities, meter)
            .map_err(StrongCompileFoundationError::LirStructure)?;

        let hir_foundation = OdrFreeHirFoundation::from_validated(hir_foundation)
            .map_err(StrongCompileFoundationError::HirOdr)?;
        let mir_foundation = OdrFreeMirFoundation::from_validated(mir_foundation)
            .map_err(StrongCompileFoundationError::MirOdr)?;
        let lir_foundation = OdrFreeLirFoundation::from_validated(lir_foundation)
            .map_err(StrongCompileFoundationError::LirOdr)?;

        Ok(OdrCheckedSingleConeCompileFoundations {
            graph,
            identities,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            lir_production,
        })
    }
}

impl OdrCheckedSingleConeCompileFoundations<'_> {
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

    pub const fn hir_production_wire(&self) -> &DecodedCoreBootstrapInterfaceSectionV1 {
        &self.hir_production
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.mir_foundation
    }

    pub const fn mir_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_production
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.lir_foundation
    }

    pub const fn lir_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        &self.lir_production
    }
}

#[derive(Debug)]
pub enum StrongCompileFoundationError {
    HirStructure(HirFoundationValidationError),
    MirStructure(MirFoundationValidationError),
    LirStructure(LirFoundationValidationError),
    HirOdr(OdrFreeHirFoundationError),
    MirOdr(OdrFreeMirFoundationError),
    LirOdr(OdrFreeLirFoundationError),
}

impl fmt::Display for StrongCompileFoundationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong Compile foundation: {self:?}")
    }
}

impl std::error::Error for StrongCompileFoundationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::HirStructure(error) => error,
            Self::MirStructure(error) => error,
            Self::LirStructure(error) => error,
            Self::HirOdr(error) => error,
            Self::MirOdr(error) => error,
            Self::LirOdr(error) => error,
        })
    }
}

fn require_strong_profile(
    graph: &ValidatedGraphArtifact<'_>,
) -> Result<ArtifactCapabilityProfile, SingleConeCompileSectionDecodeError> {
    let actual = graph.envelope.manifest().compatibility().artifact_profile();
    if actual != &ArtifactCapabilityProfileId::single_cone_strong() {
        return Err(SingleConeCompileSectionDecodeError::WrongProfile {
            actual: actual.clone(),
        });
    }
    Ok(ArtifactCapabilityProfile::SINGLE_CONE_STRONG)
}

fn metadata_member_id(
    graph: &ValidatedGraphArtifact<'_>,
    location: MetadataLocation,
) -> Result<SlibMemberId, SingleConeCompileSectionDecodeError> {
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
        .ok_or(SingleConeCompileSectionDecodeError::MissingMetadataMember { location })
}

fn member_payload<'input>(
    graph: &ValidatedGraphArtifact<'input>,
    location: MetadataLocation,
    member: SlibMemberId,
) -> Result<&'input [u8], SingleConeCompileSectionDecodeError> {
    graph.envelope.member(member).ok_or(
        SingleConeCompileSectionDecodeError::MissingMetadataMemberPayload { location, member },
    )
}

fn required_section<'input>(
    envelope: &DecodedMetadataEnvelope<'input>,
    capability: &CapabilityId,
) -> Result<&'input [u8], SingleConeCompileSectionDecodeError> {
    envelope
        .sections()
        .iter()
        .find(|section| section.capability() == capability)
        .map(DecodedMetadataSection::payload)
        .ok_or_else(|| SingleConeCompileSectionDecodeError::MissingSection {
            location: envelope.location(),
            capability: capability.clone(),
        })
}

fn decode_inner<T: WireDecode>(
    graph: &mut ValidatedGraphArtifact<'_>,
    location: MetadataLocation,
    capability: CapabilityId,
    payload: &[u8],
) -> Result<T, SingleConeCompileSectionDecodeError> {
    let meter = graph.envelope.meter_mut();
    meter
        .charge_work(COMPILE_SECTION_HANDLER_BASE_WORK, &WirePath::root())
        .map_err(|source| SingleConeCompileSectionDecodeError::InnerSection {
            location,
            capability: capability.clone(),
            source,
        })?;
    decode_canonical_with_meter(payload, meter).map_err(|source| {
        SingleConeCompileSectionDecodeError::InnerSection {
            location,
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
) -> Result<(), SingleConeCompileSectionDecodeError> {
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
    .map_err(SingleConeCompileSectionDecodeError::SemanticFingerprints)?;
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
) -> Result<(), SingleConeCompileSectionDecodeError> {
    if expected == actual {
        Ok(())
    } else {
        Err(
            SingleConeCompileSectionDecodeError::SemanticFingerprintMismatch {
                location,
                expected: *expected,
                actual: *actual,
            },
        )
    }
}

#[derive(Debug)]
pub enum SingleConeCompileSectionDecodeError {
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

impl fmt::Display for SingleConeCompileSectionDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot decode single-Cone Compile sections: {self:?}"
        )
    }
}

impl std::error::Error for SingleConeCompileSectionDecodeError {
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

#[cfg(test)]
mod tests;
