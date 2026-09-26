use scoop_identity::{CallableTemplateOrigin, PropertyOwner};

use super::*;

impl ImportedDependencySelectionPlan {
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
        if binding.sources().any(|source| {
            source.witness().terminal_declaration() != binding.target()
                || source.witness().route().terminal().exporter() != entry.provider
        }) {
            return Err(
                ImportedDependencyCandidateError::PropertyTerminalProviderMismatch {
                    property,
                    expected: entry.provider,
                },
            );
        }
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
        let capability = entry.interface.capability();
        let accessor = match accessor {
            ImportedDependencyPropertyAccessorKind::Getter => capability.getter(),
            ImportedDependencyPropertyAccessorKind::Setter => {
                let setter = capability.setter().ok_or(
                    ImportedDependencyCandidateError::MissingPropertySetter(
                        property.interface.declaration(),
                    ),
                )?;
                if capability.setter_access() != Some(crate::PropertySetterPublicAccessV1::Public) {
                    return Err(ImportedDependencyCandidateError::RestrictedPropertySetter(
                        property.interface.declaration(),
                    ));
                }
                setter
            }
        };
        self.callable_candidate_for_declaration(
            CallableTemplateOrigin::Accessor(accessor),
            &property.binding,
        )
    }
}
