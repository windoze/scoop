use scoop_identity::{CallableTemplateOrigin, PropertyOwner};

use super::*;

impl ImportedDependencySelectionPlan {
    pub fn property_definition_origin(
        &self,
        declaration: crate::PropertyDeclarationId,
    ) -> Option<&scoop_identity::DefinitionOrigin> {
        Some(
            self.catalog
                .properties
                .get(&declaration)?
                .definition_origin
                .origin(),
        )
    }

    pub fn property_native_contract(
        &self,
        property: scoop_identity::PersistentPropertyId,
    ) -> Option<std::sync::Arc<scoop_identity::SourceNativeExternalContractRecord>> {
        self.catalog
            .properties
            .get(&PropertyOwner::Property(property))?
            .native_contract
            .clone()
    }

    pub fn property_declaration(
        &self,
        declaration: crate::PropertyDeclarationId,
    ) -> Option<&crate::PropertyDeclarationRecordV1> {
        self.catalog
            .properties
            .get(&declaration)
            .map(|entry| &entry.interface)
    }

    pub fn property_for_accessor(
        &self,
        accessor: scoop_identity::PersistentPropertyAccessorId,
    ) -> Option<&crate::PropertyDeclarationRecordV1> {
        let property = self.catalog.indexes.accessors.get(&accessor)?;
        Some(&self.catalog.properties[property].interface)
    }

    pub fn property_candidate(
        &self,
        binding: &DirectImportedTargetBinding,
    ) -> Result<ImportedDependencyPropertyCandidate, ImportedDependencyCandidateError> {
        let property = match binding.target() {
            ImportedTarget::Property(id) => PropertyOwner::Property(id.persistent()),
            ImportedTarget::ExtensionProperty(id) => {
                PropertyOwner::ExtensionProperty(id.persistent())
            }
            target => return Err(ImportedDependencyCandidateError::NotProperty(target)),
        };
        let entry = self
            .catalog
            .properties
            .get(&property)
            .ok_or(ImportedDependencyCandidateError::MissingProperty(property))?;
        Ok(ImportedDependencyPropertyCandidate {
            binding: binding.clone(),
            provider: entry.provider,
            interface: entry.interface.clone(),
        })
    }

    pub fn property_accessor_candidate(
        &self,
        property: &ImportedDependencyPropertyCandidate,
        accessor: ImportedDependencyPropertyAccessorKind,
    ) -> Result<ImportedDependencyCallableCandidate, ImportedDependencyCandidateError> {
        let declaration = property.interface.declaration();
        let entry = self.catalog.properties.get(&declaration).ok_or(
            ImportedDependencyCandidateError::MissingProperty(declaration),
        )?;
        let capability = entry.interface.accessors();
        let accessor = match accessor {
            ImportedDependencyPropertyAccessorKind::Getter => capability.getter(),
            ImportedDependencyPropertyAccessorKind::Setter => capability.setter().ok_or(
                ImportedDependencyCandidateError::MissingPropertySetter(
                    property.interface.declaration(),
                ),
            )?,
        };
        self.callable_candidate_for_declaration(
            CallableTemplateOrigin::Accessor(accessor),
            &property.binding,
        )
    }
}
