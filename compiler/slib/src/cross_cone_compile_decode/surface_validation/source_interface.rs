//! Callable source protocols, default type scopes, reference closure and data flow.

use scoop_hir::CallableSourceInterfaceSetSemanticValidationError;

use super::{TypeAliasValidatedCrossConeHirFrontSections, ValidatedSurfaceFront};
use crate::cross_cone_hir_authority::{
    CrossConeHirCallableSourceAuthorityError, ValidatedNominalProviderView,
};
pub use crate::cross_cone_hir_authority::{
    CrossConeHirDefaultDataFlowError, CrossConeHirDefaultNestedIdentityError,
    CrossConeHirDefaultProviderContractError, CrossConeHirDefaultRootOriginError,
    CrossConeHirSourceInventoryError, DefaultMetadataNominalError, SourceInventoryDeclaration,
};

/// One provider whose callable source-order parameter protocol is exact and
/// whose defaults have valid root origins, source declaration contracts,
/// provider type scopes, exact body-reference closure, nested identities,
/// and local data flow.
/// Definition-side language checks are not replayed when these records are read.
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
        let mut front = self.0;
        let input = front.hir_validation_parts();
        input.sources(dependencies)?;
        Ok(SourceInterfaceValidatedCrossConeHirFrontSections(front))
    }
}

#[derive(Debug)]
pub enum CrossConeHirSourceInterfaceSurfaceError {
    SourceInventory(CrossConeHirSourceInventoryError),
    SourceInterfaces(
        CallableSourceInterfaceSetSemanticValidationError<CrossConeHirCallableSourceAuthorityError>,
    ),
    DefaultRootOrigin(CrossConeHirDefaultRootOriginError),
    DefaultProviderContract(CrossConeHirDefaultProviderContractError),
    DefaultNestedIdentity(CrossConeHirDefaultNestedIdentityError),
    DefaultDataFlow(CrossConeHirDefaultDataFlowError),
}

impl std::fmt::Display for CrossConeHirSourceInterfaceSurfaceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceInventory(error) => error.fmt(formatter),
            Self::SourceInterfaces(error) => error.fmt(formatter),
            Self::DefaultRootOrigin(error) => error.fmt(formatter),
            Self::DefaultProviderContract(error) => error.fmt(formatter),
            Self::DefaultNestedIdentity(error) => error.fmt(formatter),
            Self::DefaultDataFlow(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeHirSourceInterfaceSurfaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SourceInventory(error) => Some(error),
            Self::SourceInterfaces(error) => Some(error),
            Self::DefaultRootOrigin(error) => Some(error),
            Self::DefaultProviderContract(error) => Some(error),
            Self::DefaultNestedIdentity(error) => Some(error),
            Self::DefaultDataFlow(error) => Some(error),
        }
    }
}
