//! Closed-profile decoding before any M23-6 semantic authority is granted.

use crate::{
    ArtifactCapabilityProfile, CompileSectionDecodeError, MetadataLocation, ValidatedGraphArtifact,
    compile_sections::{decode_compile_metadata_envelopes, decode_compile_section},
    hir_core_bootstrap_interface_capability, hir_cross_cone_interface_capability,
    hir_cross_cone_type_semantics_capability, hir_identity_foundation_capability,
    lir_cone_production_capability, lir_cross_cone_layout_abi_capability,
    lir_cross_cone_param_free_bridge_capability, lir_identity_foundation_capability,
    mir_core_bootstrap_bridge_capability, mir_cross_cone_param_free_bridge_capability,
    mir_cross_cone_type_bridge_capability, mir_identity_foundation_capability,
};

use super::DecodedCrossConeLayoutCompileSections;

impl<'input> ValidatedGraphArtifact<'input> {
    /// Decodes every Compile-required payload of the exact M23-6 profile.
    /// Link-only payloads remain opaque until final-object validation.
    pub fn decode_cross_cone_layout_compile_sections(
        mut self,
    ) -> Result<
        DecodedCrossConeLayoutCompileSections<'input>,
        CrossConeLayoutCompileSectionDecodeError,
    > {
        let metadata = decode_compile_metadata_envelopes(
            &mut self,
            ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
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
        let hir_type_semantics = decode_compile_section(
            &metadata,
            MetadataLocation::Hir,
            hir_cross_cone_type_semantics_capability(),
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
        let mir_type_bridge = decode_compile_section(
            &metadata,
            MetadataLocation::Mir,
            mir_cross_cone_type_bridge_capability(),
        )?;
        let lir_foundation = decode_compile_section(
            &metadata,
            MetadataLocation::Lir,
            lir_identity_foundation_capability(),
        )?;
        let lir_strong_production = decode_compile_section(
            &metadata,
            MetadataLocation::Lir,
            lir_cone_production_capability(),
        )?;
        let lir_cross_cone_bridge = decode_compile_section(
            &metadata,
            MetadataLocation::Lir,
            lir_cross_cone_param_free_bridge_capability(),
        )?;
        let lir_layout_abi = decode_compile_section(
            &metadata,
            MetadataLocation::Lir,
            lir_cross_cone_layout_abi_capability(),
        )?;

        metadata.validate_semantic_fingerprints(&mut self)?;

        Ok(DecodedCrossConeLayoutCompileSections {
            graph: self,
            view: crate::link_decode::DecodedLayoutView::Compile,
            hir_foundation,
            hir_core_production,
            hir_interface,
            hir_type_semantics,
            mir_foundation,
            mir_core_production,
            mir_cross_cone_bridge,
            mir_type_bridge,
            lir_foundation,
            lir_strong_production,
            lir_cross_cone_bridge,
            lir_layout_abi,
        })
    }
}

pub type CrossConeLayoutCompileSectionDecodeError = CompileSectionDecodeError;
