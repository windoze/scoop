//! Per-artifact callable source protocols and default-body data flow.

use scoop_hir::CallableSourceInterfaceSetSemanticValidationError;

use super::{TypeAliasValidatedCrossConeHirFrontSections, ValidatedSurfaceFront};
use crate::cross_cone_hir_authority::{
    CanonicalCrossConeHirSurfaceAuthority, CrossConeHirCallableSourceAuthorityError,
    ValidatedNominalProviderView,
};
pub use crate::cross_cone_hir_authority::{
    CrossConeHirDefaultDataFlowError, CrossConeHirDefaultFieldError,
};

/// One provider whose callable source-order parameter protocol is exact and
/// whose default bodies have valid local data flow. Operation typing and
/// provider/reference envelopes are separate semantic checks.
pub struct SourceInterfaceValidatedCrossConeHirFrontSections<'input>(
    pub(super) ValidatedSurfaceFront<'input>,
);

impl<'input> TypeAliasValidatedCrossConeHirFrontSections<'input> {
    pub(crate) fn validate_source_interfaces<'dependency>(
        self,
        dependencies: Vec<ValidatedNominalProviderView<'dependency>>,
    ) -> Result<
        SourceInterfaceValidatedCrossConeHirFrontSections<'input>,
        CrossConeHirSourceInterfaceSurfaceError,
    > {
        let ValidatedSurfaceFront {
            mut graph,
            identities,
            foundations,
            hir_core_production,
            hir_interface,
            mir_core_production,
            mir_cross_cone_bridge,
            lir_strong_production,
            lir_cross_cone_bridge,
        } = self.0;
        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            graph.identity(),
            &identities,
            &foundations.hir,
            &hir_interface,
            dependencies,
            graph.envelope.meter_mut(),
        );
        hir_interface
            .source_interfaces()
            .validate_semantics(hir_interface.callable_interfaces(), &mut authority)
            .map_err(CrossConeHirSourceInterfaceSurfaceError::SourceInterfaces)?;
        authority
            .validate_default_local_data_flow()
            .map_err(CrossConeHirSourceInterfaceSurfaceError::DefaultDataFlow)?;
        Ok(SourceInterfaceValidatedCrossConeHirFrontSections(
            ValidatedSurfaceFront {
                graph,
                identities,
                foundations,
                hir_core_production,
                hir_interface,
                mir_core_production,
                mir_cross_cone_bridge,
                lir_strong_production,
                lir_cross_cone_bridge,
            },
        ))
    }
}

#[derive(Debug)]
pub enum CrossConeHirSourceInterfaceSurfaceError {
    SourceInterfaces(
        CallableSourceInterfaceSetSemanticValidationError<CrossConeHirCallableSourceAuthorityError>,
    ),
    DefaultDataFlow(CrossConeHirDefaultDataFlowError),
}

impl std::fmt::Display for CrossConeHirSourceInterfaceSurfaceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceInterfaces(error) => error.fmt(formatter),
            Self::DefaultDataFlow(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeHirSourceInterfaceSurfaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SourceInterfaces(error) => Some(error),
            Self::DefaultDataFlow(error) => Some(error),
        }
    }
}
