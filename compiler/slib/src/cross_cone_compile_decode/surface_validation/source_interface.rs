//! Callable source protocols, default type scopes, reference closure and data flow.

use scoop_hir::CallableSourceInterfaceSetSemanticValidationError;

use super::{TypeAliasValidatedCrossConeHirFrontSections, ValidatedSurfaceFront};
use crate::cross_cone_hir_authority::{
    CanonicalCrossConeHirSurfaceAuthority, CrossConeHirCallableSourceAuthorityError,
    ValidatedNominalProviderView,
};
pub use crate::cross_cone_hir_authority::{
    CrossConeHirDefaultDataFlowError, CrossConeHirDefaultFieldError,
    CrossConeHirDefaultNestedIdentityError, CrossConeHirDefaultProviderContractError,
    CrossConeHirDefaultRootOriginError, CrossConeHirDefaultTypeAccessError,
    CrossConeHirDefaultValueAccessError, DefaultMetadataNominalError,
};

/// One provider whose callable source-order parameter protocol is exact and
/// whose defaults have valid root origins, source declaration contracts,
/// provider type scopes, exact body-reference closure, nested identities,
/// type/value target access and local data flow.
/// Operation typing, inherited-provider
/// relations, nested ABI and reference envelopes are separate semantic checks.
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
            .validate_default_root_origins()
            .map_err(CrossConeHirSourceInterfaceSurfaceError::DefaultRootOrigin)?;
        authority
            .validate_default_provider_contracts()
            .map_err(CrossConeHirSourceInterfaceSurfaceError::DefaultProviderContract)?;
        authority
            .validate_default_nested_identities()
            .map_err(CrossConeHirSourceInterfaceSurfaceError::DefaultNestedIdentity)?;
        authority
            .validate_default_type_access(&hir_core_production)
            .map_err(CrossConeHirSourceInterfaceSurfaceError::DefaultTypeAccess)?;
        authority
            .validate_default_value_access()
            .map_err(CrossConeHirSourceInterfaceSurfaceError::DefaultValueAccess)?;
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
    DefaultRootOrigin(CrossConeHirDefaultRootOriginError),
    DefaultProviderContract(CrossConeHirDefaultProviderContractError),
    DefaultNestedIdentity(CrossConeHirDefaultNestedIdentityError),
    DefaultTypeAccess(CrossConeHirDefaultTypeAccessError),
    DefaultValueAccess(CrossConeHirDefaultValueAccessError),
    DefaultDataFlow(CrossConeHirDefaultDataFlowError),
}

impl std::fmt::Display for CrossConeHirSourceInterfaceSurfaceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceInterfaces(error) => error.fmt(formatter),
            Self::DefaultRootOrigin(error) => error.fmt(formatter),
            Self::DefaultProviderContract(error) => error.fmt(formatter),
            Self::DefaultNestedIdentity(error) => error.fmt(formatter),
            Self::DefaultTypeAccess(error) => error.fmt(formatter),
            Self::DefaultValueAccess(error) => error.fmt(formatter),
            Self::DefaultDataFlow(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeHirSourceInterfaceSurfaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SourceInterfaces(error) => Some(error),
            Self::DefaultRootOrigin(error) => Some(error),
            Self::DefaultProviderContract(error) => Some(error),
            Self::DefaultNestedIdentity(error) => Some(error),
            Self::DefaultTypeAccess(error) => Some(error),
            Self::DefaultValueAccess(error) => Some(error),
            Self::DefaultDataFlow(error) => Some(error),
        }
    }
}
