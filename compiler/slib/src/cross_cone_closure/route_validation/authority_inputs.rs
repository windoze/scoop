//! Dependency-scoped inputs shared by route-backed semantic authorities.

use scoop_hir::CanonicalPublicExportBindingsV1;
use scoop_identity::ConeIdentity;
use scoop_wire::{BudgetMeter, WireError, WirePath};

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
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Self, WireError> {
        let mut direct = Vec::new();
        reserve_route_slots(&mut direct, direct_positions.len(), meter, path)?;
        meter.charge_work(direct_positions.len() as u64, path)?;
        direct.extend(
            direct_positions
                .iter()
                .map(|position| previous[*position].identity()),
        );

        let mut providers = Vec::new();
        reserve_route_slots(&mut providers, reachable_positions.len(), meter, path)?;
        meter.charge_work(reachable_positions.len() as u64, path)?;
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
/// Charge both logical collection slots and the actual copied element bytes.
pub(crate) fn reserve_route_slots<T>(
    values: &mut Vec<T>,
    additional: usize,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), WireError> {
    meter.check_table_entries(values.len().saturating_add(additional) as u64, path)?;
    meter.charge_owned_bytes(
        (additional as u64).saturating_mul(std::mem::size_of::<T>() as u64),
        path,
    )?;
    meter.try_reserve_collection_slots(values, additional, path)
}
