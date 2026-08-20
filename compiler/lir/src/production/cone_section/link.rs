use super::*;
use crate::{LinkDataError, link_data::link_error};

impl DecodedConeProductionSectionV2 {
    /// Resolves the recorded production surface without source discovery.
    pub fn link_shape_sources(
        &self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<Vec<SourceDeclarationKey>, LinkDataError> {
        self.shape_support_plan.link_sources(identities)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn read_link(
        self,
        coordinate: ConeCoordinate,
        direct_dependencies: &[ConeIdentity],
        target: crate::LirTargetProfile,
        foundation: &ConeLirFoundation,
        identities: &mut ValidatedIdentityGraph,
        shape_sources: &[SourceDeclarationKey],
        type_definitions: &crate::StrongTypeReferenceDefinitionsV2,
        initialization_definitions: &crate::StrongInitializationDefinitionCatalogV2,
    ) -> Result<ConeProductionSectionV2, LinkDataError> {
        let entry_source = self.entry_plan.link_source(identities)?;
        self.replay(
            coordinate,
            direct_dependencies,
            target,
            foundation,
            entry_source,
            shape_sources,
            type_definitions,
            initialization_definitions,
        )
        .map_err(link_error)
    }
}
