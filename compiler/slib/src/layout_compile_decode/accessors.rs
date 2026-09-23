use scoop_hir::OdrFreeHirFoundation;
use scoop_lir::OdrFreeLirFoundation;
use scoop_mir::OdrFreeMirFoundation;
use scoop_wire::{BudgetMeter, DecodeUsage};

use super::*;
use crate::{
    ArtifactFingerprint, ConeKind, ConeSourceForm, DependencyRecord, SemanticFingerprintRecord,
};

impl<'input> DecodedCrossConeLayoutCompileSections<'input> {
    pub const fn coordinate(&self) -> &scoop_identity::ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> scoop_identity::ConeIdentity {
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

    pub const fn decode_usage(&self) -> DecodeUsage {
        self.graph.decode_usage()
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

impl IdentityCheckedCrossConeLayoutCompileSections<'_> {
    pub const fn identity(&self) -> scoop_identity::ConeIdentity {
        self.graph.identity()
    }

    pub(crate) const fn identity_graph(&self) -> &ValidatedIdentityGraph {
        &self.identities
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

impl ResolvedCrossConeLayoutHirSections<'_> {
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

    pub const fn hir_core_production_wire(&self) -> &DecodedCoreBootstrapInterfaceSectionV1 {
        &self.hir_core_production
    }

    pub const fn hir_interface(&self) -> &CrossConeHirInterfaceSectionV1 {
        &self.hir_interface
    }

    pub const fn hir_type_semantics(&self) -> &CrossConeTypeSemanticsSectionV1 {
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

impl HirProductionValidatedCrossConeLayoutSections<'_> {
    pub(crate) fn nominal_provider_view(
        &self,
    ) -> crate::cross_cone_hir_authority::ValidatedNominalProviderView<'_> {
        crate::cross_cone_hir_authority::ValidatedNominalProviderView {
            identity: self.identity(),
            identities: &self.identities,
            foundation: &self.foundations.hir,
            core: &self.hir_core_production,
            interface: &self.hir_interface,
        }
    }

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

    pub const fn hir_core_production(&self) -> &CoreBootstrapInterfaceSectionV1 {
        &self.hir_core_production
    }

    pub const fn hir_interface(&self) -> &CrossConeHirInterfaceSectionV1 {
        &self.hir_interface
    }

    pub const fn hir_type_semantics(&self) -> &CrossConeTypeSemanticsSectionV1 {
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

    pub(crate) fn hir_semantic_parts(
        &mut self,
    ) -> (
        &ValidatedIdentityGraph,
        &OdrFreeHirFoundation,
        &CoreBootstrapInterfaceSectionV1,
        &CrossConeHirInterfaceSectionV1,
        &CrossConeTypeSemanticsSectionV1,
        &mut BudgetMeter,
    ) {
        let Self {
            graph,
            identities,
            foundations,
            hir_core_production,
            hir_interface,
            hir_type_semantics,
            ..
        } = self;
        (
            identities,
            &foundations.hir,
            hir_core_production,
            hir_interface,
            hir_type_semantics,
            graph.envelope.meter_mut(),
        )
    }
}
