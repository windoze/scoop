//! Per-artifact callable source-interface validation.

use scoop_hir::CallableSourceInterfaceSetSemanticValidationError;

use super::{TypeAliasValidatedCrossConeHirFrontSections, ValidatedSurfaceFront};
use crate::cross_cone_hir_authority::{
    CanonicalCrossConeHirSurfaceAuthority, CrossConeHirCallableSourceAuthorityError,
    ValidatedNominalProviderView,
};

/// One provider whose callable source-order parameter protocol is exact.
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
            graph,
            identities,
            foundations,
            hir_core_production,
            hir_interface,
            mir_core_production,
            mir_cross_cone_bridge,
            lir_strong_production,
        } = self.0;
        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            graph.identity(),
            &identities,
            &foundations.hir,
            &hir_core_production,
            &hir_interface,
            dependencies,
        );
        hir_interface
            .source_interfaces()
            .validate_semantics(hir_interface.callable_interfaces(), &mut authority)
            .map_err(CrossConeHirSourceInterfaceSurfaceError::SourceInterfaces)?;
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
            },
        ))
    }
}

#[derive(Debug)]
pub enum CrossConeHirSourceInterfaceSurfaceError {
    SourceInterfaces(
        CallableSourceInterfaceSetSemanticValidationError<CrossConeHirCallableSourceAuthorityError>,
    ),
}

impl std::fmt::Display for CrossConeHirSourceInterfaceSurfaceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceInterfaces(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeHirSourceInterfaceSurfaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SourceInterfaces(error) => Some(error),
        }
    }
}
