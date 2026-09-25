use super::*;
use scoop_hir::{
    CrossConeHirExternalReferenceValidationError, PublicExportBindingClosureValidationError,
};
use scoop_wire::{WireError, WirePath};

use crate::cross_cone_closure::route_validation::{
    CanonicalCrossConeRouteAuthority, CrossConeHirReferenceAuthorityError, RouteProviderView,
    reserve_route_slots,
};

impl HirInterfaceValidationInput<'_> {
    pub(crate) fn references(
        self,
        direct: &[ConeIdentity],
        dependencies: &[ValidatedNominalProviderView<'_>],
        meter: &mut BudgetMeter,
    ) -> Result<(), CrossConeHirReferenceSurfaceError> {
        let path = WirePath::root();
        meter
            .charge_work(dependencies.len() as u64 + direct.len() as u64 + 1, &path)
            .map_err(CrossConeHirReferenceSurfaceError::Resource)?;
        let mut providers = Vec::new();
        reserve_route_slots(&mut providers, dependencies.len(), meter, &path)
            .map_err(CrossConeHirReferenceSurfaceError::Resource)?;
        providers.extend(dependencies.iter().map(|provider| RouteProviderView {
            identity: provider.identity,
            bindings: provider.interface.public_bindings(),
        }));
        let mut authority = CanonicalCrossConeRouteAuthority::try_new(
            self.current,
            self.identities,
            self.interface,
            direct,
            &providers,
            meter,
            &path,
        )
        .map_err(CrossConeHirReferenceSurfaceError::Resource)?;
        self.interface
            .public_bindings()
            .validate_route_closure(self.current, &authority, meter, &path.clone().field(9))
            .map_err(|error| CrossConeHirReferenceSurfaceError::Routes(Box::new(error)))?;
        self.interface
            .validate_external_reference_closure(&mut authority, meter, &path)
            .map_err(|error| CrossConeHirReferenceSurfaceError::External(Box::new(error)))?;
        self.call_sites(dependencies, meter)
            .map_err(|error| CrossConeHirReferenceSurfaceError::CallSites(Box::new(error)))?;
        self.type_sites(dependencies, meter)
            .map_err(|error| CrossConeHirReferenceSurfaceError::TypeSites(Box::new(error)))
    }
}

#[derive(Debug)]
pub enum CrossConeHirReferenceSurfaceError {
    Resource(WireError),
    CallSites(Box<CrossConeHirCallSiteOriginError>),
    TypeSites(Box<CrossConeHirTypeSiteError>),
    Routes(Box<PublicExportBindingClosureValidationError>),
    External(
        Box<CrossConeHirExternalReferenceValidationError<CrossConeHirReferenceAuthorityError>>,
    ),
}

impl std::fmt::Display for CrossConeHirReferenceSurfaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::CallSites(error) => error.fmt(f),
            Self::TypeSites(error) => error.fmt(f),
            Self::Routes(error) => error.fmt(f),
            Self::External(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for CrossConeHirReferenceSurfaceError {}
