use scoop_hir::{
    CoreBootstrapInterfaceSectionV1, CrossConeHirInterfaceSectionV1,
    CrossConeTypeSemanticsSectionV1,
};
use scoop_lir::{ConeLirFoundation, CrossConeLayoutAbiSectionV1, CrossConeLirBridgeSectionV1};
use scoop_mir::{
    CoreBootstrapBridgeSectionV1, CrossConeMirBridgeSectionV1, CrossConeMirTypeBridgeSectionV1,
};

use super::CrossConeLayoutArtifactWriteError;
use crate::strong_artifact::{LayerAssembly, StrongArtifactSectionV1, build_section};
use crate::{
    CrossConeLayoutLinkClosureSectionV1, CrossConeLinkClosureSectionV1,
    LinkIdentityClosureSectionV1, MemberPurposeSet, MetadataLocation, VerifiedCodeFingerprintV2,
    hir_core_bootstrap_interface_capability, hir_cross_cone_interface_capability,
    hir_cross_cone_type_semantics_capability, hir_identity_foundation_capability,
    lir_cone_production_capability, lir_cross_cone_layout_abi_capability,
    lir_cross_cone_layout_link_closure_capability, lir_cross_cone_link_closure_capability,
    lir_cross_cone_param_free_bridge_capability, lir_identity_foundation_capability,
    lir_link_identity_closure_capability, mir_core_bootstrap_bridge_capability,
    mir_cross_cone_param_free_bridge_capability, mir_cross_cone_type_bridge_capability,
    mir_identity_foundation_capability,
};

pub(super) struct LayoutMetadataInput<'section, 'ir> {
    pub hir_foundation: &'section scoop_hir::CanonicalHirFoundation,
    pub hir_production: &'section CoreBootstrapInterfaceSectionV1,
    pub hir_cross_cone: CrossConeHirInterfaceSectionV1,
    pub hir_type_semantics: &'section CrossConeTypeSemanticsSectionV1,
    pub mir_foundation: &'section scoop_mir::CanonicalMirFoundation,
    pub mir_production: &'section CoreBootstrapBridgeSectionV1,
    pub mir_cross_cone: &'section CrossConeMirBridgeSectionV1,
    pub mir_type_bridge: &'section CrossConeMirTypeBridgeSectionV1<'ir>,
    pub lir_foundation: &'section ConeLirFoundation,
    pub lir_cross_cone: &'section CrossConeLirBridgeSectionV1,
    pub lir_layout_abi: &'section CrossConeLayoutAbiSectionV1<'ir>,
    pub code: &'section VerifiedCodeFingerprintV2,
    pub link_identity_closure: &'section LinkIdentityClosureSectionV1,
    pub callable_link_closure: &'section CrossConeLinkClosureSectionV1,
    pub layout_link_closure: &'section CrossConeLayoutLinkClosureSectionV1<'ir>,
    pub link_support: &'section crate::LirLinkSupportSectionV1,
}

pub(super) struct LayoutMetadataAssembly {
    pub hir: LayerAssembly,
    pub mir: LayerAssembly,
    pub lir: LayerAssembly,
}

pub(super) fn assemble_metadata(
    mut input: LayoutMetadataInput<'_, '_>,
) -> Result<LayoutMetadataAssembly, CrossConeLayoutArtifactWriteError> {
    let hir_cross_cone = input
        .hir_cross_cone
        .index_for_wire()
        .map_err(CrossConeLayoutArtifactWriteError::HirInterfaceIndex)?;
    let hir = LayerAssembly::new(
        MetadataLocation::Hir,
        vec![
            build_section(
                StrongArtifactSectionV1::HirFoundation,
                MetadataLocation::Hir,
                hir_identity_foundation_capability(),
                MemberPurposeSet::COMPILE_AND_LINK,
                input.hir_foundation,
            )?,
            build_section(
                StrongArtifactSectionV1::HirProduction,
                MetadataLocation::Hir,
                hir_core_bootstrap_interface_capability(),
                MemberPurposeSet::COMPILE,
                input.hir_production,
            )?,
            build_section(
                StrongArtifactSectionV1::HirCrossConeInterface,
                MetadataLocation::Hir,
                hir_cross_cone_interface_capability(),
                MemberPurposeSet::COMPILE,
                &hir_cross_cone,
            )?,
            build_section(
                StrongArtifactSectionV1::HirCrossConeTypeSemantics,
                MetadataLocation::Hir,
                hir_cross_cone_type_semantics_capability(),
                MemberPurposeSet::COMPILE,
                input.hir_type_semantics,
            )?,
        ],
    )?;
    let mir = LayerAssembly::new(
        MetadataLocation::Mir,
        vec![
            build_section(
                StrongArtifactSectionV1::MirFoundation,
                MetadataLocation::Mir,
                mir_identity_foundation_capability(),
                MemberPurposeSet::COMPILE_AND_LINK,
                input.mir_foundation,
            )?,
            build_section(
                StrongArtifactSectionV1::MirProduction,
                MetadataLocation::Mir,
                mir_core_bootstrap_bridge_capability(),
                MemberPurposeSet::COMPILE,
                input.mir_production,
            )?,
            build_section(
                StrongArtifactSectionV1::MirCrossConeBridge,
                MetadataLocation::Mir,
                mir_cross_cone_param_free_bridge_capability(),
                MemberPurposeSet::COMPILE,
                input.mir_cross_cone,
            )?,
            build_section(
                StrongArtifactSectionV1::MirCrossConeTypeBridge,
                MetadataLocation::Mir,
                mir_cross_cone_type_bridge_capability(),
                MemberPurposeSet::COMPILE,
                input.mir_type_bridge,
            )?,
        ],
    )?;
    let lir = LayerAssembly::new(
        MetadataLocation::Lir,
        vec![
            build_section(
                StrongArtifactSectionV1::LinkSupport,
                MetadataLocation::Lir,
                crate::lir_link_support_capability(),
                MemberPurposeSet::COMPILE_AND_LINK,
                input.link_support,
            )?,
            build_section(
                StrongArtifactSectionV1::LirFoundation,
                MetadataLocation::Lir,
                lir_identity_foundation_capability(),
                MemberPurposeSet::COMPILE_AND_LINK,
                input.lir_foundation,
            )?,
            build_section(
                StrongArtifactSectionV1::LirCrossConeBridge,
                MetadataLocation::Lir,
                lir_cross_cone_param_free_bridge_capability(),
                MemberPurposeSet::COMPILE_AND_LINK,
                input.lir_cross_cone,
            )?,
            build_section(
                StrongArtifactSectionV1::LirProduction,
                MetadataLocation::Lir,
                lir_cone_production_capability(),
                MemberPurposeSet::COMPILE_AND_LINK,
                input.code.production().strong_production(),
            )?,
            build_section(
                StrongArtifactSectionV1::LirCrossConeLayoutAbi,
                MetadataLocation::Lir,
                lir_cross_cone_layout_abi_capability(),
                MemberPurposeSet::COMPILE_AND_LINK,
                input.lir_layout_abi,
            )?,
            build_section(
                StrongArtifactSectionV1::LinkIdentityClosure,
                MetadataLocation::Lir,
                lir_link_identity_closure_capability(),
                MemberPurposeSet::LINK,
                input.link_identity_closure,
            )?,
            build_section(
                StrongArtifactSectionV1::CrossConeLinkClosure,
                MetadataLocation::Lir,
                lir_cross_cone_link_closure_capability(),
                MemberPurposeSet::LINK,
                input.callable_link_closure,
            )?,
            build_section(
                StrongArtifactSectionV1::CrossConeLayoutLinkClosure,
                MetadataLocation::Lir,
                lir_cross_cone_layout_link_closure_capability(),
                MemberPurposeSet::LINK,
                input.layout_link_closure,
            )?,
        ],
    )?;
    Ok(LayoutMetadataAssembly { hir, mir, lir })
}
