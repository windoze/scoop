use super::*;

impl<'input> PreparedCrossConeLayoutMirSections<'input> {
    pub(crate) fn code_strong_input(
        &mut self,
    ) -> Result<scoop_lir::DecodedConeProductionSectionV2, crate::SharedLirPhysicalError> {
        crate::link_decode::layout_code_strong_input(&mut self.graph)
            .map_err(|error| crate::LayoutLinkSymbolUseError::CodeInput(Box::new(error)).into())
    }

    pub(crate) fn replay_link_object_contents(
        &mut self,
        strong: &scoop_lir::ConeProductionSectionV2,
        profile: &scoop_lir::CBridgeToolchainProfileV1,
    ) -> Result<crate::ReplayedLayoutLinkObjectContentsV1, crate::SharedLirPhysicalError> {
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
    ) -> Result<(), crate::StrongLinkMaterializationError> {
        if let Some(link) = self.view.link() {
            let partition =
                scoop_lir::ProducerUnitPartitionV1::from_foundation(&self.foundations.lir)
                    .map_err(crate::StrongLinkMaterializationError::ProducerUnits)?;
            let plan = link
                .link_identity_closure_wire()
                .replay_materializations(&partition)
                .map_err(crate::StrongLinkMaterializationError::Closure)?;
            crate::link_decode::object_directory::validate(&mut self.graph, &plan)?;
        }
        Ok(())
    }
}
