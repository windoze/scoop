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
        if binding
            .sources()
            .any(|source| source.immediate_provider().brand() != self.catalog.world_brand)
        {
            return Err(ImportedDependencyCandidateError::ForeignWorld);
        }
        let entry = self
            .catalog
            .properties
            .get(&property)
            .ok_or(ImportedDependencyCandidateError::MissingProperty(property))?;
        if binding.sources().any(|source| {
            source.witness().terminal_declaration() != binding.target()
                || source.witness().route().terminal().exporter() != entry.certificate.identity()
        }) {
            return Err(
                ImportedDependencyCandidateError::PropertyTerminalProviderMismatch {
                    property,
                    expected: entry.certificate.identity(),
                },
            );
        }
        Ok(ImportedDependencyPropertyCandidate {
            projection: self.catalog.projection,
            binding: binding.clone(),
            certificate: entry.certificate.clone(),
            interface: entry.interface.clone(),
        })
    }

    pub fn property_accessor_candidate(
        &self,
        property: &ImportedDependencyPropertyCandidate,
        accessor: ImportedDependencyPropertyAccessorKind,
    ) -> Result<ImportedDependencyCallableCandidate, ImportedDependencyCandidateError> {
        if property.projection != self.catalog.projection {
            return Err(ImportedDependencyCandidateError::ForeignProjection);
        }
        let capability = property.interface.capability();
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
