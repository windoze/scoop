//! Per-artifact strong LIR production and dependency-bridge validation.

use scoop_hir::{CoreBootstrapInterfaceSectionV1, CrossConeHirInterfaceSectionV1};
use scoop_identity::{
    CanonicalScoopAbiFunctionSignature, ConeCoordinate, ConeIdentity, ExactCallableSignature,
    GcEffect, ValidatedIdentityGraph,
};
use scoop_lir::{
    ConeLirFoundation, ConeProductionSectionV1, CrossConeLirBridgeSectionV1,
    CrossConeLirBridgeValidationError,
};
use scoop_mir::{CoreBootstrapBridgeSectionV1, CrossConeMirBridgeSectionV1, OdrFreeMirFoundation};

use super::MirBridgeValidatedCrossConeHirFrontSections;
use crate::{
    NativeBoundaryCompileError, ValidatedGraphArtifact,
    compile_decode::{
        NativeBoundaryFoundationView, replay_canonical_scoop_abi,
        validate_shared_native_boundary_parts,
    },
    strong_compile_decode::{
        OdrFreeStrongFoundationSet, StrongProfileLirProductionError, StrongProfileSemanticFront,
        validate_strong_profile_lir_with_shape_sources,
    },
};

mod native_boundary;

/// One provider whose complete local strong production and compile-facing LIR
/// dependency bridge were independently rebuilt from validated foundations.
/// Native representation replay and cross-provider MIR/LIR equality remain
/// closure-level obligations.
pub struct LirBridgeValidatedCrossConeHirFrontSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    hir_core_production: CoreBootstrapInterfaceSectionV1,
    hir_interface: CrossConeHirInterfaceSectionV1,
    mir_core_production: CoreBootstrapBridgeSectionV1,
    mir_cross_cone_bridge: CrossConeMirBridgeSectionV1,
    lir_strong_production: ConeProductionSectionV1,
    lir_cross_cone_bridge: CrossConeLirBridgeSectionV1,
}

mod production;
pub use production::ValidatedCrossConeSemanticsProduction;

impl<'input> LirBridgeValidatedCrossConeHirFrontSections<'input> {
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

    pub fn hir_core_production(&self) -> &CoreBootstrapInterfaceSectionV1 {
        &self.hir_core_production
    }

    pub const fn hir_interface(&self) -> &CrossConeHirInterfaceSectionV1 {
        &self.hir_interface
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub fn mir_core_production(&self) -> &CoreBootstrapBridgeSectionV1 {
        &self.mir_core_production
    }

    pub const fn mir_cross_cone_bridge(&self) -> &CrossConeMirBridgeSectionV1 {
        &self.mir_cross_cone_bridge
    }

    pub const fn lir_foundation(&self) -> &ConeLirFoundation {
        &self.foundations.lir
    }

    pub fn lir_strong_production(&self) -> &ConeProductionSectionV1 {
        &self.lir_strong_production
    }

    pub const fn lir_cross_cone_bridge(&self) -> &CrossConeLirBridgeSectionV1 {
        &self.lir_cross_cone_bridge
    }

    pub(crate) fn replay_canonical_scoop_abi<'a>(
        &mut self,
        dependencies: impl ExactSizeIterator<Item = crate::AbiReplayDependency<'a>>,
        signature: &ExactCallableSignature,
        gc_effect: GcEffect,
    ) -> Result<CanonicalScoopAbiFunctionSignature, NativeBoundaryCompileError> {
        let current = crate::AbiReplayDependency {
            identity: self.graph.identity(),
            identities: &self.identities,
            foundation: &self.foundations.hir,
            nominals: self.hir_interface.nominal_interfaces(),
        };
        replay_canonical_scoop_abi(&mut self.graph, current, dependencies, signature, gc_effect)
    }

    pub(crate) fn abi_replay_types(&self) -> crate::AbiReplayDependency<'_> {
        crate::AbiReplayDependency {
            identity: self.graph.identity(),
            identities: &self.identities,
            foundation: &self.foundations.hir,
            nominals: self.hir_interface.nominal_interfaces(),
        }
    }

    pub(crate) fn append_abi_expectations(
        &mut self,
        expectations: &mut Vec<crate::AbiExpectation>,
    ) -> Result<(), crate::CrossConeLirClosureRelationError> {
        crate::validate_local_projection(
            self.graph.identity(),
            &self.hir_interface,
            &self.mir_cross_cone_bridge,
            &self.lir_cross_cone_bridge,
            expectations,
        )
    }
}

impl<'input> MirBridgeValidatedCrossConeHirFrontSections<'input> {
    pub(crate) fn validate_lir_bridge(
        self,
    ) -> Result<LirBridgeValidatedCrossConeHirFrontSections<'input>, CrossConeLirFrontValidationError>
    {
        let Self {
            graph,
            mut identities,
            foundations,
            hir_core_production,
            hir_interface,
            mir_core_production,
            mir_cross_cone_bridge,
            lir_strong_production,
            lir_cross_cone_bridge,
        } = self;
        let shape_sources = scoop_hir::PublicNominalShapeRequirementsV1::from_shared_surface(
            graph.identity(),
            hir_core_production.direct_public_surface(),
            foundations.hir.as_canonical(),
            hir_interface.nominal_interfaces(),
            hir_interface.callable_interfaces(),
        )
        .and_then(|roots| roots.source_declarations(foundations.hir.as_canonical()))
        .map_err(StrongProfileLirProductionError::ShapeSources)
        .map_err(CrossConeLirFrontValidationError::StrongProduction)?;
        let lir_strong_production = validate_strong_profile_lir_with_shape_sources(
            &graph,
            &mut identities,
            StrongProfileSemanticFront {
                hir_foundation: &foundations.hir,
                hir_production: &hir_core_production,
                mir_production: &mir_core_production,
                lir_foundation: &foundations.lir,
            },
            lir_strong_production,
            &shape_sources,
        )
        .map_err(CrossConeLirFrontValidationError::StrongProduction)?;
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
            Self::StrongProduction(source) => source,
            Self::NativeBoundary(source) => source,
            Self::DependencyBridge(source) => source,
        })
    }
}
