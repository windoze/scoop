//! Provider implementations are independent of source name lookup entries.

use std::sync::Arc;

use super::{ImportedDependencyDefinitionSource, ImportedDependencyDefinitionSources};

#[derive(Clone, Debug)]
pub struct ImportedCallableBody {
    pub(super) body: Arc<crate::ExportGenericCallableBodyV1>,
    pub(super) definition_sources: Arc<ImportedDependencyDefinitionSources>,
    pub(super) source_name: Option<scoop_identity::CanonicalIdentifier>,
}

impl ImportedCallableBody {
    pub fn body(&self) -> &crate::ExportGenericCallableBodyV1 {
        &self.body
    }

    /// Named source functions have names; compiler-generated bodies do not.
    pub fn source_name(&self) -> Option<&scoop_identity::CanonicalIdentifier> {
        self.source_name.as_ref()
    }

    pub fn definition_source(
        &self,
        source: &crate::ExportDefinitionSourceV1,
    ) -> Option<ImportedDependencyDefinitionSource<'_>> {
        self.definition_sources.resolve(source)
    }
}

impl super::ImportedDependencySelectionPlan {
    pub fn callable_body(
        &self,
        owner: crate::DefaultCallableDeclarationV1,
    ) -> Option<ImportedCallableBody> {
        self.catalog.bodies.get(&owner).cloned()
    }
}
