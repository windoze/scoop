//! Closed-profile section decoding before semantic validation.

use crate::{
    ArtifactCapabilityProfile, CompileSectionDecodeError, MetadataLocation, ValidatedGraphArtifact,
    compile_sections::{decode_compile_metadata_envelopes, decode_compile_section},
    hir_core_bootstrap_interface_capability, hir_cross_cone_interface_capability,
    hir_identity_foundation_capability, lir_cross_cone_param_free_bridge_capability,
    lir_identity_foundation_capability, lir_strong_production_capability,
    mir_core_bootstrap_bridge_capability, mir_cross_cone_param_free_bridge_capability,
    mir_identity_foundation_capability,
};

use super::DecodedCrossConeHirFrontSections;

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
            &metadata,
            MetadataLocation::Hir,
            hir_identity_foundation_capability(),
        )?;
        let hir_core_production = decode_compile_section(
            &metadata,
            MetadataLocation::Hir,
            hir_core_bootstrap_interface_capability(),
        )?;
        let hir_interface = decode_compile_section(
            &metadata,
            MetadataLocation::Hir,
            hir_cross_cone_interface_capability(),
        )?;
        let mir_foundation = decode_compile_section(
            &metadata,
            MetadataLocation::Mir,
            mir_identity_foundation_capability(),
        )?;
        let mir_core_production = decode_compile_section(
            &metadata,
            MetadataLocation::Mir,
            mir_core_bootstrap_bridge_capability(),
        )?;
        let mir_cross_cone_bridge = decode_compile_section(
            &metadata,
            MetadataLocation::Mir,
            mir_cross_cone_param_free_bridge_capability(),
        )?;
        let lir_foundation = decode_compile_section(
            &metadata,
            MetadataLocation::Lir,
            lir_identity_foundation_capability(),
        )?;
        let lir_strong_production = decode_compile_section(
            &metadata,
            MetadataLocation::Lir,
            lir_strong_production_capability(),
        )?;
        let lir_cross_cone_bridge = decode_compile_section(
            &metadata,
            MetadataLocation::Lir,
            lir_cross_cone_param_free_bridge_capability(),
        )?;

        metadata.validate_semantic_fingerprints(&mut self)?;

        Ok(DecodedCrossConeHirFrontSections {
            graph: self,
            hir_foundation,
            hir_core_production,
            hir_interface,
            mir_foundation,
            mir_core_production,
            mir_cross_cone_bridge,
            lir_foundation,
            lir_strong_production,
            lir_cross_cone_bridge,
        })
    }
}

pub type CrossConeHirFrontSectionDecodeError = CompileSectionDecodeError;
