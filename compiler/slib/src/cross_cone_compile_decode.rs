//! Compile-view HIR-front decoding for the cross-Cone semantics profile.

use scoop_hir::{
    DecodedCoreBootstrapInterfaceSectionV1, DecodedCrossConeHirInterfaceSectionV1,
    DecodedHirFoundation,
};
use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_lir::{DecodedLirFoundation, DecodedStrongProductionSectionV1};
use scoop_mir::{DecodedCoreBootstrapBridgeSectionV1, DecodedMirFoundation};
use scoop_wire::DecodeUsage;

use crate::{
    ArtifactCapabilityProfile, ArtifactFingerprint, CompileSectionDecodeError, MetadataLocation,
    ValidatedGraphArtifact,
    compile_sections::{decode_compile_metadata_envelopes, decode_compile_section},
    hir_core_bootstrap_interface_capability, hir_cross_cone_interface_capability,
    hir_identity_foundation_capability, lir_identity_foundation_capability,
    lir_strong_production_capability, mir_core_bootstrap_bridge_capability,
    mir_identity_foundation_capability,
};

/// Canonically decoded identity, legacy production, and general HIR payloads
/// from one exact `cross-cone-semantics-strong/1` Compile view.
///
/// The new MIR/LIR bridge payloads are intentionally not promoted by this
/// state. They remain obligations of the later bridge-validation phase, so
/// this type is neither a complete per-artifact Compile proof nor a semantic
/// closure proof.
#[derive(Debug)]
pub struct DecodedCrossConeHirFrontSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    hir_foundation: DecodedHirFoundation,
    hir_core_production: DecodedCoreBootstrapInterfaceSectionV1,
    hir_interface: DecodedCrossConeHirInterfaceSectionV1,
    mir_foundation: DecodedMirFoundation,
    mir_core_production: DecodedCoreBootstrapBridgeSectionV1,
    lir_foundation: DecodedLirFoundation,
    lir_strong_production: DecodedStrongProductionSectionV1,
}

impl<'input> ValidatedGraphArtifact<'input> {
    /// Opens the HIR-facing front of the M23-5 profile without granting any
    /// identity, surface, route, bridge, or session-import authority.
    pub fn decode_cross_cone_hir_front_sections(
        mut self,
    ) -> Result<DecodedCrossConeHirFrontSections<'input>, CrossConeHirFrontSectionDecodeError> {
        let metadata = decode_compile_metadata_envelopes(
            &mut self,
            ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
        )?;

        let hir_foundation = decode_compile_section(
            &mut self,
            &metadata,
            MetadataLocation::Hir,
            hir_identity_foundation_capability(),
        )?;
        let hir_core_production = decode_compile_section(
            &mut self,
            &metadata,
            MetadataLocation::Hir,
            hir_core_bootstrap_interface_capability(),
        )?;
        let hir_interface = decode_compile_section(
            &mut self,
            &metadata,
            MetadataLocation::Hir,
            hir_cross_cone_interface_capability(),
        )?;
        let mir_foundation = decode_compile_section(
            &mut self,
            &metadata,
            MetadataLocation::Mir,
            mir_identity_foundation_capability(),
        )?;
        let mir_core_production = decode_compile_section(
            &mut self,
            &metadata,
            MetadataLocation::Mir,
            mir_core_bootstrap_bridge_capability(),
        )?;
        let lir_foundation = decode_compile_section(
            &mut self,
            &metadata,
            MetadataLocation::Lir,
            lir_identity_foundation_capability(),
        )?;
        let lir_strong_production = decode_compile_section(
            &mut self,
            &metadata,
            MetadataLocation::Lir,
            lir_strong_production_capability(),
        )?;

        metadata.validate_semantic_fingerprints(&mut self)?;

        Ok(DecodedCrossConeHirFrontSections {
            graph: self,
            hir_foundation,
            hir_core_production,
            hir_interface,
            mir_foundation,
            mir_core_production,
            lir_foundation,
            lir_strong_production,
        })
    }
}

impl DecodedCrossConeHirFrontSections<'_> {
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

    pub const fn hir_core_production_wire(&self) -> &DecodedCoreBootstrapInterfaceSectionV1 {
        &self.hir_core_production
    }

    pub const fn hir_interface_wire(&self) -> &DecodedCrossConeHirInterfaceSectionV1 {
        &self.hir_interface
    }

    pub const fn mir_foundation_wire(&self) -> &DecodedMirFoundation {
        &self.mir_foundation
    }

    pub const fn mir_core_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_core_production
    }

    pub const fn lir_foundation_wire(&self) -> &DecodedLirFoundation {
        &self.lir_foundation
    }

    pub const fn lir_strong_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        &self.lir_strong_production
    }
}

pub type CrossConeHirFrontSectionDecodeError = CompileSectionDecodeError;

#[cfg(test)]
mod tests;
