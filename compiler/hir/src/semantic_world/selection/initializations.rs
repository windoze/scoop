use std::sync::Arc;

use super::{ImportedDependencyDefinitionSource, ImportedDependencyDefinitionSources};

#[derive(Clone, Debug)]
pub struct ImportedNominalInitialization {
    pub(super) initialization: Arc<crate::ExportGenericNominalInitializationV1>,
    pub(super) definition_sources: Arc<ImportedDependencyDefinitionSources>,
}

impl ImportedNominalInitialization {
    pub fn initialization(&self) -> &Arc<crate::ExportGenericNominalInitializationV1> {
        &self.initialization
    }

    pub fn source_location(
        &self,
        source: &scoop_identity::SourceIdentity,
        context: scoop_identity::PersistentSourceContextId,
    ) -> Option<ImportedDependencyDefinitionSource<'_>> {
        self.definition_sources.resolve_location(source, context)
    }

    pub fn definition_source(
        &self,
        source: &crate::ExportDefinitionSourceV1,
    ) -> Option<ImportedDependencyDefinitionSource<'_>> {
        self.definition_sources.resolve(source)
    }
}

impl super::ImportedDependencySelectionPlan {
    pub fn nominal_initialization(
        &self,
        owner: scoop_identity::PersistentGenericTypeId,
    ) -> Option<ImportedNominalInitialization> {
        self.catalog.initializations.get(&owner).cloned()
    }
}
