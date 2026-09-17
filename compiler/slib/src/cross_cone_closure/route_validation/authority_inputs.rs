//! Dependency-scoped inputs shared by route-backed semantic authorities.

use scoop_hir::CanonicalPublicExportBindingsV1;
use scoop_identity::ConeIdentity;

use crate::ConstValidatedCrossConeHirFrontSections;

#[derive(Clone, Copy)]
pub(in crate::cross_cone_closure) struct RouteProviderView<'a> {
    pub(in crate::cross_cone_closure) identity: ConeIdentity,
    pub(in crate::cross_cone_closure) bindings: &'a CanonicalPublicExportBindingsV1,
}

pub(in crate::cross_cone_closure) struct RouteAuthorityInputs<'a> {
    direct: Vec<ConeIdentity>,
    providers: Vec<RouteProviderView<'a>>,
    closure_node_count: usize,
}

impl<'a> RouteAuthorityInputs<'a> {
    pub(in crate::cross_cone_closure) fn try_new(
        previous: &'a [ConstValidatedCrossConeHirFrontSections<'_>],
        direct_positions: &[usize],
        reachable_positions: &[usize],
    ) -> Result<Self, usize> {
        let mut direct = Vec::new();
        direct
            .try_reserve_exact(direct_positions.len())
            .map_err(|_| direct_positions.len())?;
        direct.extend(
            direct_positions
                .iter()
                .map(|position| previous[*position].identity()),
        );

        let mut providers = Vec::new();
        providers
            .try_reserve_exact(reachable_positions.len())
            .map_err(|_| reachable_positions.len())?;
        providers.extend(
            reachable_positions
                .iter()
                .map(|position| RouteProviderView {
                    identity: previous[*position].identity(),
                    bindings: previous[*position].hir_interface().public_bindings(),
                }),
        );
        let closure_node_count = reachable_positions.len().checked_add(1).ok_or(usize::MAX)?;

        Ok(Self {
            direct,
            providers,
            closure_node_count,
        })
    }

    pub(in crate::cross_cone_closure) fn direct(&self) -> &[ConeIdentity] {
        &self.direct
    }

    pub(in crate::cross_cone_closure) fn providers(&self) -> &[RouteProviderView<'a>] {
        &self.providers
    }

    pub(in crate::cross_cone_closure) const fn closure_node_count(&self) -> usize {
        self.closure_node_count
    }
}
