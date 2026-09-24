use super::*;
use scoop_wire::WirePath;

impl LirBridgeValidatedCrossConeHirFrontSections<'_> {
    pub(crate) fn validate_shared_native_boundary(
        &mut self,
        previous: &[LirBridgeValidatedCrossConeHirFrontSections<'_>],
        dependency_positions: &[Vec<usize>],
    ) -> Result<(), NativeBoundaryCompileError> {
        let meter = self.graph.envelope.meter_mut();
        let reachable = crate::dependency_reachability::transitive_positions(
            previous.len(),
            dependency_positions,
            meter,
        )
        .map_err(NativeBoundaryCompileError::Resource)?;
        let mut dependencies = Vec::new();
        meter
            .try_reserve_collection_slots(&mut dependencies, reachable.len(), &WirePath::root())
            .map_err(NativeBoundaryCompileError::Resource)?;
        dependencies.extend(
            reachable
                .iter()
                .map(|position| previous[*position].abi_replay_types()),
        );
        let current = crate::AbiReplayDependency {
            identity: self.graph.identity(),
            identities: &self.identities,
            foundation: &self.foundations.hir,
            nominals: self.hir_interface.nominal_interfaces(),
        };
        validate_shared_native_boundary_parts(
            &mut self.graph,
            current,
            &dependencies,
            &NativeBoundaryFoundationView::from_odr_free(
                &self.foundations.hir,
                &self.foundations.mir,
                &self.foundations.lir,
            ),
            self.foundations.lir.materialized_exact_types(),
        )
    }
}
