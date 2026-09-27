use super::*;
use crate::{
    CrossConeHirCallableSurfaceError, CrossConeHirNominalSurfaceError,
    CrossConeHirPropertySurfaceError, CrossConeHirTypeAliasSurfaceError,
};

impl<'a> HirInterfaceValidationInput<'a> {
    pub(crate) fn nominals(
        self,
        dependencies: Vec<ValidatedNominalProviderView<'a>>,
    ) -> Result<(), CrossConeHirNominalSurfaceError> {
        self.interface
            .nominal_interfaces()
            .validate_declared_field_inventory(self.foundation.as_canonical())
            .map_err(CrossConeHirNominalSurfaceError::Fields)?;
        self.interface
            .nominal_interfaces()
            .validate_declared_relation_inventory(self.foundation.as_canonical())
            .map_err(CrossConeHirNominalSurfaceError::Relations)?;
        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            self.current,
            self.identities,
            self.foundation,
            self.interface,
            dependencies,
        );
        self.interface
            .nominal_interfaces()
            .validate_semantics(&mut authority)
            .map_err(|error| CrossConeHirNominalSurfaceError::NominalInterfaces(Box::new(error)))?;
        self.interface
            .nominal_interfaces()
            .validate_support_semantics(&mut authority)
            .map_err(|error| CrossConeHirNominalSurfaceError::NominalInterfaces(Box::new(error)))?;
        Ok(())
    }
    pub(crate) fn properties(
        self,
        dependencies: Vec<ValidatedNominalProviderView<'a>>,
    ) -> Result<(), CrossConeHirPropertySurfaceError> {
        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            self.current,
            self.identities,
            self.foundation,
            self.interface,
            dependencies,
        );
        self.interface
            .property_interfaces()
            .validate_semantics(&mut authority)
            .map_err(|error| {
                CrossConeHirPropertySurfaceError::PropertyInterfaces(Box::new(error))
            })?;
        self.interface
            .property_interfaces()
            .validate_support_semantics(&mut authority)
            .map_err(|error| {
                CrossConeHirPropertySurfaceError::PropertyInterfaces(Box::new(error))
            })?;
        authority
            .validate_support_property_origins()
            .map_err(CrossConeHirPropertySurfaceError::Declarations)?;
        Ok(())
    }
    pub(crate) fn callables(
        self,
        dependencies: Vec<ValidatedNominalProviderView<'a>>,
    ) -> Result<(), CrossConeHirCallableSurfaceError> {
        self.interface
            .nominal_interfaces()
            .validate_dispatch_declarations(self.interface.callable_interfaces())
            .map_err(CrossConeHirCallableSurfaceError::Dispatch)?;
        crate::cross_cone_hir_authority::validate_intrinsic_declarations(
            self.interface,
            self.identities,
            std::iter::once((self.current, self.core)).chain(
                dependencies
                    .iter()
                    .map(|provider| (provider.identity, provider.core)),
            ),
        )
        .map_err(|error| CrossConeHirCallableSurfaceError::Intrinsics(Box::new(error)))?;
        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            self.current,
            self.identities,
            self.foundation,
            self.interface,
            dependencies,
        );
        self.interface
            .callable_interfaces()
            .validate_semantics(&mut authority)
            .map_err(|error| {
                CrossConeHirCallableSurfaceError::CallableInterfaces(Box::new(error))
            })?;
        self.interface
            .callable_interfaces()
            .validate_support_semantics(&mut authority)
            .map_err(|error| {
                CrossConeHirCallableSurfaceError::CallableInterfaces(Box::new(error))
            })?;
        authority
            .validate_support_callable_origins()
            .map_err(CrossConeHirCallableSurfaceError::Declarations)?;
        authority
            .validate_property_setter_domains()
            .map_err(CrossConeHirCallableSurfaceError::Declarations)?;
        Ok(())
    }
    pub(crate) fn type_aliases(
        self,
        dependencies: Vec<ValidatedNominalProviderView<'a>>,
    ) -> Result<(), CrossConeHirTypeAliasSurfaceError> {
        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            self.current,
            self.identities,
            self.foundation,
            self.interface,
            dependencies,
        );
        self.interface
            .type_aliases()
            .validate_semantics(&mut authority)
            .map_err(|error| {
                CrossConeHirTypeAliasSurfaceError::TypeAliasInterfaces(Box::new(error))
            })?;
        Ok(())
    }
}
