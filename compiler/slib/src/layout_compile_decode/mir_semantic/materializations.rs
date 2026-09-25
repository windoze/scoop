use super::*;

impl PreparedCrossConeLayoutMirSections<'_> {
    pub(crate) fn validate_link_materializations(
        &mut self,
        strong: &scoop_lir::ReplayedStrongProductionSectionV2,
    ) -> Result<(), crate::StrongLinkMaterializationError> {
        if let Some(link) = self.view.link() {
            let partition = crate::link_decode::materializations::producer_units(
                &self.foundations.lir,
                strong.generated_bridge_plan(),
                self.graph.envelope.meter_mut(),
            )?;
            let plan = link
                .link_identity_closure_wire()
                .replay_materializations(&partition, self.graph.envelope.meter_mut())
                .map_err(crate::StrongLinkMaterializationError::Closure)?;
            crate::link_decode::object_directory::validate(&mut self.graph, &plan)?;
        }
        Ok(())
    }
}
