use scoop_identity::{CallableTemplateOrigin, PropertyOwner};

use super::*;

impl ImportedDependencySelectionPlan {
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
        self.catalog.properties.values().find_map(|property| {
            let capability = property.interface.accessors();
            (capability.getter() == accessor || capability.setter() == Some(accessor))
                .then_some(&property.interface)
        })
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
