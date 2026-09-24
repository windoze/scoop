use super::*;
use scoop_hir::{
    CrossConeHirExternalReferenceValidationError, PublicExportBindingClosureValidationError,
};
use scoop_wire::{WireError, WirePath};

use crate::cross_cone_closure::route_validation::{
    CanonicalCrossConeRouteAuthority, CrossConeHirReferenceAuthorityError, RouteProviderView,
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
        providers
            .try_reserve_exact(dependencies.len())
            .map_err(|_| CrossConeHirReferenceSurfaceError::Allocation {
                requested_slots: dependencies.len(),
            })?;
        providers.extend(dependencies.iter().map(|provider| RouteProviderView {
            identity: provider.identity,
            bindings: provider.interface.public_bindings(),
        }));
        let closure_node_count = dependencies.len().checked_add(1).ok_or(
            CrossConeHirReferenceSurfaceError::Allocation {
                requested_slots: usize::MAX,
            },
        )?;
        let mut authority = CanonicalCrossConeRouteAuthority::try_new(
            self.current,
            self.identities,
            self.interface,
            direct,
            &providers,
            closure_node_count,
        )
        .map_err(
            |requested_slots| CrossConeHirReferenceSurfaceError::Allocation { requested_slots },
        )?;
        self.interface
            .public_bindings()
            .validate_route_closure(self.current, &authority)
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
    Allocation {
        requested_slots: usize,
    },
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
            Self::Allocation { requested_slots } => {
                write!(
                    f,
                    "cannot allocate {requested_slots} HIR reference provider slots"
                )
            }
            Self::Resource(error) => error.fmt(f),
            Self::CallSites(error) => error.fmt(f),
            Self::TypeSites(error) => error.fmt(f),
            Self::Routes(error) => error.fmt(f),
            Self::External(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for CrossConeHirReferenceSurfaceError {}
