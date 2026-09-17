//! Per-artifact strong LIR production and dependency-bridge validation.

use scoop_hir::{CoreBootstrapInterfaceSectionV1, CrossConeHirInterfaceSectionV1};
use scoop_identity::{
    CanonicalScoopAbiFunctionSignature, ConeCoordinate, ConeIdentity, ExactCallableSignature,
    GcEffect, ValidatedIdentityGraph,
};
use scoop_lir::{
    CrossConeLirBridgeSectionV1, CrossConeLirBridgeValidationError, OdrFreeLirFoundation,
    StrongExternalLirBridgeReconstructionError, StrongProductionSectionV1,
};
use scoop_mir::{CoreBootstrapBridgeSectionV1, CrossConeMirBridgeSectionV1, OdrFreeMirFoundation};

use super::MirBridgeValidatedCrossConeHirFrontSections;
use crate::{
    NativeBoundaryCompileError, ValidatedGraphArtifact,
    compile_decode::{
        NativeBoundaryFoundationView, replay_canonical_scoop_abi, validate_native_boundary_parts,
    },
    strong_compile_decode::{
        OdrFreeStrongFoundationSet, StrongProfileLirProductionError, StrongProfileSemanticFront,
        validate_strong_profile_lir_production,
    },
};

/// One provider whose complete local strong production and compile-facing LIR
/// dependency bridge were independently rebuilt from validated foundations.
/// Cross-provider MIR/LIR equality remains a closure-level obligation.
pub struct LirBridgeValidatedCrossConeHirFrontSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    hir_core_production: CoreBootstrapInterfaceSectionV1,
    hir_interface: CrossConeHirInterfaceSectionV1,
    mir_core_production: CoreBootstrapBridgeSectionV1,
    mir_cross_cone_bridge: CrossConeMirBridgeSectionV1,
    lir_strong_production: StrongProductionSectionV1,
    lir_cross_cone_bridge: CrossConeLirBridgeSectionV1,
}

impl LirBridgeValidatedCrossConeHirFrontSections<'_> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
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

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn mir_core_production(&self) -> &CoreBootstrapBridgeSectionV1 {
        &self.mir_core_production
    }

    pub const fn mir_cross_cone_bridge(&self) -> &CrossConeMirBridgeSectionV1 {
        &self.mir_cross_cone_bridge
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundations.lir
    }

    pub const fn lir_strong_production(&self) -> &StrongProductionSectionV1 {
        &self.lir_strong_production
    }

    pub const fn lir_cross_cone_bridge(&self) -> &CrossConeLirBridgeSectionV1 {
        &self.lir_cross_cone_bridge
    }

    pub(crate) fn replay_canonical_scoop_abi(
        &mut self,
        signature: &ExactCallableSignature,
        gc_effect: GcEffect,
    ) -> Result<CanonicalScoopAbiFunctionSignature, NativeBoundaryCompileError> {
        replay_canonical_scoop_abi(
            &mut self.graph,
            &self.identities,
            &self.foundations.hir,
            signature,
            gc_effect,
        )
    }
}

impl<'input> MirBridgeValidatedCrossConeHirFrontSections<'input> {
    pub(crate) fn validate_lir_bridge(
        self,
    ) -> Result<LirBridgeValidatedCrossConeHirFrontSections<'input>, CrossConeLirFrontValidationError>
    {
        let Self {
            mut graph,
            mut identities,
            foundations,
            hir_core_production,
            hir_interface,
            mir_core_production,
            mir_cross_cone_bridge,
            lir_strong_production,
            lir_cross_cone_bridge,
        } = self;
        let external_bridges = lir_strong_production
            .reconstruct_external_bridges(graph.identity(), &mut identities)
            .map_err(CrossConeLirFrontValidationError::ExternalBridges)?;
        let lir_strong_production = validate_strong_profile_lir_production(
            &graph,
            &mut identities,
            StrongProfileSemanticFront {
                hir_foundation: &foundations.hir,
                hir_production: &hir_core_production,
                mir_production: &mir_core_production,
                lir_foundation: &foundations.lir,
            },
            lir_strong_production,
            &external_bridges,
        )
        .map_err(CrossConeLirFrontValidationError::StrongProduction)?;
        validate_native_boundary_parts(
            &mut graph,
            &identities,
            &NativeBoundaryFoundationView::from_odr_free(
                &foundations.hir,
                &foundations.mir,
                &foundations.lir,
            ),
        )
        .map_err(CrossConeLirFrontValidationError::NativeBoundary)?;
        let lir_cross_cone_bridge = lir_cross_cone_bridge
            .validate(&mut identities, &foundations.lir)
            .map_err(CrossConeLirFrontValidationError::DependencyBridge)?;

        Ok(LirBridgeValidatedCrossConeHirFrontSections {
            graph,
            identities,
            foundations,
            hir_core_production,
            hir_interface,
            mir_core_production,
            mir_cross_cone_bridge,
            lir_strong_production,
            lir_cross_cone_bridge,
        })
    }
}

#[derive(Debug)]
pub enum CrossConeLirFrontValidationError {
    ExternalBridges(StrongExternalLirBridgeReconstructionError),
    StrongProduction(StrongProfileLirProductionError),
    NativeBoundary(NativeBoundaryCompileError),
    DependencyBridge(CrossConeLirBridgeValidationError),
}

impl std::fmt::Display for CrossConeLirFrontValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid cross-Cone LIR front: {self:?}")
    }
}

impl std::error::Error for CrossConeLirFrontValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::ExternalBridges(source) => source,
            Self::StrongProduction(source) => source,
            Self::NativeBoundary(source) => source,
            Self::DependencyBridge(source) => source,
        })
    }
}
