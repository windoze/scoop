//! Per-artifact MIR production and ordinary-dependency bridge validation.

use scoop_hir::{CoreBootstrapInterfaceSectionV1, CrossConeHirInterfaceSectionV1};
use scoop_identity::{ConeCoordinate, ConeIdentity, ValidatedIdentityGraph};
use scoop_lir::{
    DecodedCrossConeLirBridgeSectionV1, DecodedStrongProductionSectionV1, OdrFreeLirFoundation,
};
use scoop_mir::{
    CoreBootstrapBridgeSectionV1, CrossConeMirBridgeSectionV1, CrossConeMirBridgeValidationError,
    MirProductionValidationError, OdrFreeMirFoundation,
};

use super::{ConstValidatedCrossConeHirFrontSections, ValidatedSurfaceFront};
use crate::{
    ValidatedGraphArtifact,
    strong_compile_decode::{
        OdrFreeStrongFoundationSet, StrongProfileRelationError, validate_strong_profile_relations,
    },
};

/// One provider whose legacy MIR production and M23-5 dependency bridge are
/// structurally valid. Closure-wide export eligibility and selected-provider
/// matching are represented by the later closure type-state.
pub struct MirBridgeValidatedCrossConeHirFrontSections<'input> {
    pub(super) graph: ValidatedGraphArtifact<'input>,
    pub(super) identities: ValidatedIdentityGraph,
    pub(super) foundations: OdrFreeStrongFoundationSet,
    pub(super) hir_core_production: CoreBootstrapInterfaceSectionV1,
    pub(super) hir_interface: CrossConeHirInterfaceSectionV1,
    pub(super) mir_core_production: CoreBootstrapBridgeSectionV1,
    pub(super) mir_cross_cone_bridge: CrossConeMirBridgeSectionV1,
    pub(super) lir_strong_production: DecodedStrongProductionSectionV1,
    pub(super) lir_cross_cone_bridge: DecodedCrossConeLirBridgeSectionV1,
}

impl MirBridgeValidatedCrossConeHirFrontSections<'_> {
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

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundations.lir
    }

    pub const fn mir_core_production(&self) -> &CoreBootstrapBridgeSectionV1 {
        &self.mir_core_production
    }

    pub const fn mir_cross_cone_bridge(&self) -> &CrossConeMirBridgeSectionV1 {
        &self.mir_cross_cone_bridge
    }

    pub const fn lir_strong_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        &self.lir_strong_production
    }

    pub const fn lir_cross_cone_bridge_wire(&self) -> &DecodedCrossConeLirBridgeSectionV1 {
        &self.lir_cross_cone_bridge
    }

    pub(crate) fn mir_export_validation_parts(
        &mut self,
    ) -> (
        &CrossConeHirInterfaceSectionV1,
        &scoop_mir::StrongCallableBridgeSurfaceV1,
        &CrossConeMirBridgeSectionV1,
    ) {
        (
            &self.hir_interface,
            self.mir_core_production.strong_callable_bridges(),
            &self.mir_cross_cone_bridge,
        )
    }
}

impl<'input> ConstValidatedCrossConeHirFrontSections<'input> {
    pub(crate) fn validate_mir_bridge(
        self,
    ) -> Result<MirBridgeValidatedCrossConeHirFrontSections<'input>, CrossConeMirFrontValidationError>
    {
        let ValidatedSurfaceFront {
            graph,
            mut identities,
            foundations,
            hir_core_production,
            hir_interface,
            mir_core_production,
            mir_cross_cone_bridge,
            lir_strong_production,
            lir_cross_cone_bridge,
        } = self.0;
        let mir_core_production = mir_core_production
            .validate_against(graph.identity(), &mut identities, &foundations.mir)
            .map_err(CrossConeMirFrontValidationError::CoreProduction)?;
        validate_strong_profile_relations(
            graph.identity(),
            graph.kind(),
            &hir_core_production,
            &mir_core_production,
            &foundations.hir,
        )
        .map_err(CrossConeMirFrontValidationError::CrossLayer)?;
        let mir_cross_cone_bridge = mir_cross_cone_bridge
            .validate(graph.identity(), &mut identities, &foundations.mir)
            .map_err(CrossConeMirFrontValidationError::DependencyBridge)?;
        crate::hir_dependency_calls::validate_executable_hir_calls(
            &hir_interface,
            mir_core_production.strong_callable_bridges(),
            &mir_cross_cone_bridge,
            None,
        )
        .map_err(|source| CrossConeMirFrontValidationError::CallSites(Box::new(source)))?;

        Ok(MirBridgeValidatedCrossConeHirFrontSections {
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
pub enum CrossConeMirFrontValidationError {
    CallSites(Box<crate::CrossConeMirClosureRelationError>),
    CoreProduction(MirProductionValidationError),
    CrossLayer(StrongProfileRelationError),
    DependencyBridge(CrossConeMirBridgeValidationError),
}

impl std::fmt::Display for CrossConeMirFrontValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CallSites(error) => write!(formatter, "invalid HIR/MIR call sites: {error}"),
            Self::CoreProduction(error) => {
                write!(formatter, "invalid legacy MIR production: {error}")
            }
            Self::CrossLayer(error) => {
                write!(formatter, "invalid HIR/MIR production relation: {error}")
            }
            Self::DependencyBridge(error) => {
                write!(formatter, "invalid ordinary-dependency MIR bridge: {error}")
            }
        }
    }
}

impl std::error::Error for CrossConeMirFrontValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CallSites(error) => Some(error.as_ref()),
            Self::CoreProduction(error) => Some(error),
            Self::CrossLayer(error) => Some(error),
            Self::DependencyBridge(error) => Some(error),
        }
    }
}
