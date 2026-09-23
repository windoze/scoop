//! Identity resolution for both HIR transports in the M23-6 profile.

use std::fmt;

use scoop_hir::{
    CoreBootstrapInterfaceValidationError, CrossConeHirInterfaceResolutionError,
    TypeSemanticsSectionResolutionError,
};
use scoop_identity::IdentityReferenceError;
use scoop_wire::WirePath;

use super::{
    FoundationValidatedCrossConeLayoutCompileSections,
    HirProductionValidatedCrossConeLayoutSections, ResolvedCrossConeLayoutHirSections,
};

impl<'input> FoundationValidatedCrossConeLayoutCompileSections<'input> {
    /// Resolves the old public interface and the new type-semantics transport
    /// against one foundation identity graph and one artifact budget.
    pub fn resolve_hir_sections(
        self,
    ) -> Result<ResolvedCrossConeLayoutHirSections<'input>, CrossConeLayoutHirResolutionError> {
        let Self {
            mut graph,
            mut identities,
            foundations,
            hir_core_production,
            hir_interface,
            hir_type_semantics,
            mir_core_production,
            mir_cross_cone_bridge,
            mir_type_bridge,
            lir_strong_production,
            lir_cross_cone_bridge,
            lir_layout_abi,
        } = self;
        let hir_interface = hir_interface
            .resolve_metered(&mut identities, graph.envelope.meter_mut())
            .map_err(|error| CrossConeLayoutHirResolutionError::Public(Box::new(error)))?;
        let hir_type_semantics = hir_type_semantics
            .resolve(
                &mut identities,
                graph.envelope.meter_mut(),
                &WirePath::root(),
            )
            .map_err(|error| CrossConeLayoutHirResolutionError::TypeSemantics(Box::new(error)))?;
        Ok(ResolvedCrossConeLayoutHirSections {
            graph,
            identities,
            foundations,
            hir_core_production,
            hir_interface,
            hir_type_semantics,
            mir_core_production,
            mir_cross_cone_bridge,
            mir_type_bridge,
            lir_strong_production,
            lir_cross_cone_bridge,
            lir_layout_abi,
        })
    }
}

impl<'input> ResolvedCrossConeLayoutHirSections<'input> {
    /// Replays the unchanged core-bootstrap HIR contract before either the
    /// old public surface or the new type-semantics tables are trusted.
    pub fn validate_hir_production(
        self,
    ) -> Result<
        HirProductionValidatedCrossConeLayoutSections<'input>,
        CoreBootstrapInterfaceValidationError,
    > {
        let Self {
            graph,
            identities,
            foundations,
            hir_core_production,
            hir_interface,
            hir_type_semantics,
            mir_core_production,
            mir_cross_cone_bridge,
            mir_type_bridge,
            lir_strong_production,
            lir_cross_cone_bridge,
            lir_layout_abi,
        } = self;
        let hir_core_production = hir_core_production
            .validate_against_strong_foundation(graph.identity(), &foundations.hir)?;
        Ok(HirProductionValidatedCrossConeLayoutSections {
            graph,
            identities,
            foundations,
            hir_core_production,
            hir_interface,
            hir_type_semantics,
            mir_core_production,
            mir_cross_cone_bridge,
            mir_type_bridge,
            lir_strong_production,
            lir_cross_cone_bridge,
            lir_layout_abi,
        })
    }
}

#[derive(Debug)]
pub enum CrossConeLayoutHirResolutionError {
    Public(Box<CrossConeHirInterfaceResolutionError<IdentityReferenceError>>),
    TypeSemantics(Box<TypeSemanticsSectionResolutionError<IdentityReferenceError>>),
}

impl fmt::Display for CrossConeLayoutHirResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Public(error) => {
                write!(formatter, "cannot resolve public HIR interface: {error}")
            }
            Self::TypeSemantics(error) => {
                write!(formatter, "cannot resolve HIR type semantics: {error}")
            }
        }
    }
}

impl std::error::Error for CrossConeLayoutHirResolutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Public(error) => Some(error.as_ref()),
            Self::TypeSemantics(error) => Some(error.as_ref()),
        }
    }
}
