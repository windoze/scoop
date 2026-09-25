//! Dependency-scoped inputs shared by route-backed semantic authorities.

use scoop_hir::CanonicalPublicExportBindingsV1;
use scoop_identity::ConeIdentity;
use scoop_wire::{WireError, WirePath};

use crate::ConstValidatedCrossConeHirFrontSections;

#[derive(Clone, Copy)]
pub(crate) struct RouteProviderView<'a> {
    pub(crate) identity: ConeIdentity,
    pub(crate) bindings: &'a CanonicalPublicExportBindingsV1,
}

pub(in crate::cross_cone_closure) struct RouteAuthorityInputs<'a> {
    direct: Vec<ConeIdentity>,
    providers: Vec<RouteProviderView<'a>>,
}

impl<'a> RouteAuthorityInputs<'a> {
    pub(in crate::cross_cone_closure) fn try_new(
        previous: &'a [ConstValidatedCrossConeHirFrontSections<'_>],
        direct_positions: &[usize],
        reachable_positions: &[usize],

        path: &WirePath,
    ) -> Result<Self, WireError> {
        let mut direct = Vec::new();
        reserve_route_slots(&mut direct, direct_positions.len(), path)?;

        direct.extend(
            direct_positions
                .iter()
                .map(|position| previous[*position].identity()),
        );

        let mut providers = Vec::new();
        reserve_route_slots(&mut providers, reachable_positions.len(), path)?;

        providers.extend(
            reachable_positions
                .iter()
                .map(|position| RouteProviderView {
                    identity: previous[*position].identity(),
                    bindings: previous[*position].hir_interface().public_bindings(),
                }),
        );

        Ok(Self { direct, providers })
    }

    pub(in crate::cross_cone_closure) fn direct(&self) -> &[ConeIdentity] {
        &self.direct
    }

    pub(in crate::cross_cone_closure) fn providers(&self) -> &[RouteProviderView<'a>] {
        &self.providers
    }
}

/// Temporary route directories retain typed ids and borrowed canonical keys.
pub(crate) fn reserve_route_slots<T>(
    values: &mut Vec<T>,
    additional: usize,

    path: &WirePath,
) -> Result<(), WireError> {
    scoop_wire::allocation::try_reserve(values, additional, path)
}
