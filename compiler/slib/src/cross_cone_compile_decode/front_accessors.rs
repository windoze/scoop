//! Read-only observations exposed by the HIR-front type states.

use scoop_hir::{
    CoreBootstrapInterfaceSectionV1, CrossConeHirInterfaceSectionV1,
    DecodedCoreBootstrapInterfaceSectionV1, DecodedCrossConeHirInterfaceSectionV1,
    DecodedHirFoundation, OdrFreeHirFoundation,
};
use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_lir::{
    ConeLirFoundation, DecodedConeProductionSectionV1, DecodedCrossConeLirBridgeSectionV1,
    DecodedLirFoundation,
};
use scoop_mir::{
    DecodedCoreBootstrapBridgeSectionV1, DecodedCrossConeMirBridgeSectionV1, DecodedMirFoundation,
    OdrFreeMirFoundation,
};

use crate::{
    ArtifactFingerprint, ConeKind, ConeSourceForm, DependencyRecord, SemanticFingerprintRecord,
};

use super::{
    DecodedCrossConeHirFrontSections, FoundationValidatedCrossConeHirFrontSections,
    HirProductionValidatedCrossConeHirFrontSections, ResolvedCrossConeHirFrontSections,
};

impl DecodedCrossConeHirFrontSections<'_> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
    }

    pub const fn kind(&self) -> ConeKind {
        self.graph.kind()
    }

    pub const fn source_form(&self) -> ConeSourceForm {
        self.graph.source_form()
    }

    pub const fn target_selection(&self) -> scoop_lir::ValidatedLirTargetSelection {
        self.graph.target_selection()
    }

    pub fn direct_dependencies(&self) -> &[DependencyRecord] {
        self.graph.direct_dependencies()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.graph.artifact_fingerprint()
    }

    pub const fn semantic_fingerprints(&self) -> SemanticFingerprintRecord {
        self.graph.envelope.manifest().semantic_fingerprints()
    }

    pub fn dependency_record(&self) -> DependencyRecord {
        let semantic = self.semantic_fingerprints();
        DependencyRecord::from_validated(
            self.coordinate().clone(),
            self.identity(),
            semantic.hir(),
            semantic.mir(),
            semantic.lir(),
        )
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

    pub const fn mir_cross_cone_bridge_wire(&self) -> &DecodedCrossConeMirBridgeSectionV1 {
        &self.mir_cross_cone_bridge
    }

    pub const fn lir_foundation_wire(&self) -> &DecodedLirFoundation {
        &self.lir_foundation
    }

    pub const fn lir_strong_production_wire(&self) -> &DecodedConeProductionSectionV1 {
        &self.lir_strong_production
    }

    pub const fn lir_cross_cone_bridge_wire(&self) -> &DecodedCrossConeLirBridgeSectionV1 {
        &self.lir_cross_cone_bridge
    }
}

impl FoundationValidatedCrossConeHirFrontSections<'_> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
    }

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &ConeLirFoundation {
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

    pub const fn mir_core_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_core_production
    }

    pub const fn mir_cross_cone_bridge_wire(&self) -> &DecodedCrossConeMirBridgeSectionV1 {
        &self.mir_cross_cone_bridge
    }

    pub const fn lir_strong_production_wire(&self) -> &DecodedConeProductionSectionV1 {
        &self.lir_strong_production
    }

    pub const fn lir_cross_cone_bridge_wire(&self) -> &DecodedCrossConeLirBridgeSectionV1 {
        &self.lir_cross_cone_bridge
    }
}

impl ResolvedCrossConeHirFrontSections<'_> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
    }

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &ConeLirFoundation {
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

    pub const fn hir_interface(&self) -> &CrossConeHirInterfaceSectionV1 {
        &self.hir_interface
    }

    pub const fn mir_core_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_core_production
    }

    pub const fn mir_cross_cone_bridge_wire(&self) -> &DecodedCrossConeMirBridgeSectionV1 {
        &self.mir_cross_cone_bridge
    }

    pub const fn lir_strong_production_wire(&self) -> &DecodedConeProductionSectionV1 {
        &self.lir_strong_production
    }

    pub const fn lir_cross_cone_bridge_wire(&self) -> &DecodedCrossConeLirBridgeSectionV1 {
        &self.lir_cross_cone_bridge
    }
}

impl HirProductionValidatedCrossConeHirFrontSections<'_> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
    }

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &ConeLirFoundation {
        &self.foundations.lir
    }

    pub fn identity_count(&self) -> usize {
        self.identities.identity_count()
    }

    pub fn declared_identity_count(&self) -> usize {
        self.identities.declared_identity_count()
    }

    pub const fn hir_core_production(&self) -> &CoreBootstrapInterfaceSectionV1 {
        &self.hir_core_production
    }

    pub const fn hir_interface(&self) -> &CrossConeHirInterfaceSectionV1 {
        &self.hir_interface
    }

    pub const fn mir_core_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_core_production
    }

    pub const fn mir_cross_cone_bridge_wire(&self) -> &DecodedCrossConeMirBridgeSectionV1 {
        &self.mir_cross_cone_bridge
    }

    pub const fn lir_strong_production_wire(&self) -> &DecodedConeProductionSectionV1 {
        &self.lir_strong_production
    }

    pub const fn lir_cross_cone_bridge_wire(&self) -> &DecodedCrossConeLirBridgeSectionV1 {
        &self.lir_cross_cone_bridge
    }
}
