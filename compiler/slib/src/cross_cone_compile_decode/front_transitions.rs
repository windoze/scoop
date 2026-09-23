//! Identity, foundation, and legacy-HIR type-state transitions.

use scoop_hir::{CoreBootstrapInterfaceValidationError, CrossConeHirInterfaceResolutionError};
use scoop_identity::{IdentityReferenceError, IdentityValidationError, ValidatedIdentityGraph};

use crate::{
    compile_decode::validate_foundation_identity_graph_with_authorities,
    strong_compile_decode::{
        StrongProfileFoundationError, validate_cross_cone_strong_profile_foundations,
    },
};

use super::{
    DecodedCrossConeHirFrontSections, FoundationValidatedCrossConeHirFrontSections,
    HirProductionValidatedCrossConeHirFrontSections, ResolvedCrossConeHirFrontSections,
};

impl<'input> DecodedCrossConeHirFrontSections<'input> {
    /// Validates this artifact's complete foundation identity delta against
    /// only the already validated authority of its own dependency closure.
    pub(crate) fn validate_foundation_identities<'authority>(
        &mut self,
        external_authorities: impl IntoIterator<Item = &'authority ValidatedIdentityGraph>,
    ) -> Result<ValidatedIdentityGraph, IdentityValidationError> {
        validate_foundation_identity_graph_with_authorities(
            &mut self.graph,
            &self.hir_foundation,
            &self.mir_foundation,
            &self.lir_foundation,
            external_authorities,
        )
    }

    pub(crate) fn validate_foundation_structure(
        self,
        mut identities: ValidatedIdentityGraph,
    ) -> Result<FoundationValidatedCrossConeHirFrontSections<'input>, StrongProfileFoundationError>
    {
        let Self {
            mut graph,
            hir_foundation,
            hir_core_production,
            hir_interface,
            mir_foundation,
            mir_core_production,
            mir_cross_cone_bridge,
            lir_foundation,
            lir_strong_production,
            lir_cross_cone_bridge,
        } = self;
        let foundations = validate_cross_cone_strong_profile_foundations(
            &mut graph,
            &mut identities,
            hir_foundation,
            mir_foundation,
            lir_foundation,
        )?;
        Ok(FoundationValidatedCrossConeHirFrontSections {
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

impl<'input> FoundationValidatedCrossConeHirFrontSections<'input> {
    pub(crate) fn resolve_hir_interface(
        self,
    ) -> Result<
        ResolvedCrossConeHirFrontSections<'input>,
        CrossConeHirInterfaceResolutionError<IdentityReferenceError>,
    > {
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
        let hir_interface =
            hir_interface.resolve_metered(&mut identities, graph.envelope.meter_mut())?;
        Ok(ResolvedCrossConeHirFrontSections {
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

impl<'input> ResolvedCrossConeHirFrontSections<'input> {
    /// Replays the unchanged M23-3 HIR production contract against the same
    /// ODR-free foundation used to resolve the general interface. This grants
    /// the canonical direct-public surface needed by the M23-5 semantic pass.
    pub(crate) fn validate_hir_production(
        self,
    ) -> Result<
        HirProductionValidatedCrossConeHirFrontSections<'input>,
        CoreBootstrapInterfaceValidationError,
    > {
        let Self {
            graph,
            identities,
            foundations,
            hir_core_production,
            hir_interface,
            mir_core_production,
            mir_cross_cone_bridge,
            lir_strong_production,
            lir_cross_cone_bridge,
        } = self;
        let hir_core_production = hir_core_production
            .validate_against_strong_foundation(graph.identity(), &foundations.hir)?;
        Ok(HirProductionValidatedCrossConeHirFrontSections {
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
