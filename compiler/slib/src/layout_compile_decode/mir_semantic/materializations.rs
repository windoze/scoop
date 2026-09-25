use super::*;

impl<'input> PreparedCrossConeLayoutMirSections<'input> {
    pub(crate) fn code_strong_input(
        &mut self,
    ) -> Result<scoop_lir::DecodedStrongProductionSectionV2, crate::SharedLirPhysicalError> {
        crate::link_decode::layout_code_strong_input(&mut self.graph)
            .map_err(|error| crate::LayoutLinkSymbolUseError::CodeInput(Box::new(error)).into())
    }

    pub(crate) fn replay_link_object_contents(
        &mut self,
        strong: &scoop_lir::ReplayedStrongProductionSectionV2,
        profile: &scoop_lir::CBridgeToolchainProfileV1,
    ) -> Result<crate::ReplayedLayoutLinkObjectContentsV1<'input>, crate::SharedLirPhysicalError>
    {
        let link = self
            .view
            .link()
            .ok_or(crate::LayoutLinkObjectContentsError::CompileView)?;
        crate::layout_link_objects::replay(
            link,
            &mut self.graph,
            &self.foundations.lir,
            strong,
            profile,
        )
    }

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
