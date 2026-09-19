use scoop_hir::OdrFreeHirFoundation;
use scoop_lir::OdrFreeLirFoundation;
use scoop_mir::OdrFreeMirFoundation;
use scoop_wire::DecodeUsage;

use super::*;
use crate::{ArtifactFingerprint, DependencyRecord};

impl<'input> DecodedCrossConeLayoutCompileSections<'input> {
    pub const fn coordinate(&self) -> &scoop_identity::ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> scoop_identity::ConeIdentity {
        self.graph.identity()
    }

    pub fn direct_dependencies(&self) -> &[DependencyRecord] {
        self.graph.direct_dependencies()
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

    pub const fn hir_type_semantics_wire(&self) -> &DecodedCrossConeTypeSemanticsSectionV1 {
        &self.hir_type_semantics
    }

    pub const fn mir_foundation_wire(&self) -> &DecodedMirFoundation {
        &self.mir_foundation
    }

    pub const fn mir_core_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_core_production
    }

    pub const fn mir_cross_cone_bridge_wire(&self) -> &DecodedCrossConeMirBridgeSectionV1 {
        &self.mir_cross_cone_bridge
    }

    pub const fn mir_type_bridge_wire(&self) -> &DecodedCrossConeMirTypeBridgeSectionV1 {
        &self.mir_type_bridge
    }

    pub const fn lir_foundation_wire(&self) -> &DecodedLirFoundation {
        &self.lir_foundation
    }

    pub const fn lir_strong_production_wire(&self) -> &DecodedStrongProductionSectionV2 {
        &self.lir_strong_production
    }

    pub const fn lir_cross_cone_bridge_wire(&self) -> &DecodedCrossConeLirBridgeSectionV1 {
        &self.lir_cross_cone_bridge
    }

    pub const fn lir_layout_abi_wire(&self) -> &DecodedCrossConeLayoutAbiSectionV1 {
        &self.lir_layout_abi
    }
}

impl FoundationValidatedCrossConeLayoutCompileSections<'_> {
    pub const fn coordinate(&self) -> &scoop_identity::ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> scoop_identity::ConeIdentity {
        self.graph.identity()
    }

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundations.lir
    }

    pub fn identity_count(&self) -> usize {
        self.identities.identity_count()
    }

    pub fn declared_identity_count(&self) -> usize {
        self.identities.declared_identity_count()
    }

    pub const fn hir_core_production_wire(&self) -> &DecodedCoreBootstrapInterfaceSectionV1 {
        &self.hir_core_production
    }

    pub const fn hir_interface_wire(&self) -> &DecodedCrossConeHirInterfaceSectionV1 {
        &self.hir_interface
    }

    pub const fn hir_type_semantics_wire(&self) -> &DecodedCrossConeTypeSemanticsSectionV1 {
        &self.hir_type_semantics
    }

    pub const fn mir_core_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_core_production
    }

    pub const fn mir_cross_cone_bridge_wire(&self) -> &DecodedCrossConeMirBridgeSectionV1 {
        &self.mir_cross_cone_bridge
    }

    pub const fn mir_type_bridge_wire(&self) -> &DecodedCrossConeMirTypeBridgeSectionV1 {
        &self.mir_type_bridge
    }

    pub const fn lir_strong_production_wire(&self) -> &DecodedStrongProductionSectionV2 {
        &self.lir_strong_production
    }

    pub const fn lir_cross_cone_bridge_wire(&self) -> &DecodedCrossConeLirBridgeSectionV1 {
        &self.lir_cross_cone_bridge
    }

    pub const fn lir_layout_abi_wire(&self) -> &DecodedCrossConeLayoutAbiSectionV1 {
        &self.lir_layout_abi
    }
}
