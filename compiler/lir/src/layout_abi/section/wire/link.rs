use super::*;
use crate::{LinkDataError, link_data::link_error};
use scoop_identity::{
    ConeCoordinate, ExactTypeDiagnosticCatalog, SourceDeclarationKey, ValidatedIdentityGraph,
};

impl DecodedCrossConeLayoutAbiSectionV1 {
    pub fn read_link_layouts(
        self,
        target: crate::LirTargetProfile,
        foundation: &crate::ConeLirFoundation,
        identities: &mut ValidatedIdentityGraph,
        dependencies: &[&crate::CanonicalExactLayoutExportsV1],
    ) -> Result<LayoutsResolvedCrossConeLayoutAbiSectionV1, LinkDataError> {
        Ok(LayoutsResolvedCrossConeLayoutAbiSectionV1 {
            layouts: self
                .layouts
                .read_link(target, foundation, identities, dependencies)?,
            descriptors: self.descriptors,
            dispatch: self.dispatch,
            callables: self.callables,
            shape_support: self.shape_support,
            selected: self.selected,
        })
    }
}

impl LayoutsResolvedCrossConeLayoutAbiSectionV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn read_link_exports(
        self,
        target: crate::LirTargetProfile,
        foundation: &crate::ConeLirFoundation,
        identities: &mut ValidatedIdentityGraph,
        production: &crate::ConeProductionSectionV2,
        ordinary: &crate::CrossConeLirBridgeSectionV1,
        shape_sources: &[SourceDeclarationKey],
        coordinates: &[ConeCoordinate],
        dependencies: &[&LayoutAbiExportConstituentsV1],
    ) -> Result<ExportsResolvedCrossConeLayoutAbiSectionV1, LinkDataError> {
        let callables = self.callables.read_link(target, foundation, identities)?;
        let dispatch = self.dispatch.read_link(
            target,
            foundation,
            identities,
            &self.layouts,
            &callables,
            ordinary,
            dependencies,
        )?;
        let diagnostics =
            ExactTypeDiagnosticCatalog::try_new(identities, coordinates).map_err(link_error)?;
        let descriptors = self.descriptors.read_link(
            target,
            foundation,
            &self.layouts,
            production,
            &diagnostics,
        )?;
        let shapes = self
            .shape_support
            .validate(shape_sources, &self.layouts, &descriptors, foundation)
            .map_err(link_error)?;
        let exports = LayoutAbiExportConstituentsV1::try_new(
            self.layouts,
            descriptors,
            dispatch,
            callables,
            shapes,
            ordinary.clone(),
        )
        .map_err(link_error)?;
        Ok(ExportsResolvedCrossConeLayoutAbiSectionV1 {
            exports,
            selected: self.selected,
        })
    }
}
