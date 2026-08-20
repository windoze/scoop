use scoop_identity::{CallableTemplateOrigin, DefinitionOrigin, PropertyOwner};

use super::{ImportedDependencySelectionPlan, ImportedTarget};
use crate::SourceNominalId;

impl ImportedDependencySelectionPlan {
    /// Return the declaration's own source, including through public reexports.
    pub fn definition_origin(&self, target: ImportedTarget) -> Option<&DefinitionOrigin> {
        let origin = match target {
            ImportedTarget::Type(id) => {
                &self
                    .catalog
                    .nominals
                    .get(&SourceNominalId::Concrete(id.persistent()))?
                    .origin
            }
            ImportedTarget::GenericType(id) => {
                &self
                    .catalog
                    .nominals
                    .get(&SourceNominalId::GenericTemplate(id.persistent()))?
                    .origin
            }
            ImportedTarget::ObjectValue(id) => &self.singleton_owner(id.persistent())?.origin,
            ImportedTarget::Function(id) => {
                &self
                    .catalog
                    .callables
                    .get(&CallableTemplateOrigin::Function(id.persistent()))?
                    .definition_origin
            }
            ImportedTarget::GenericFunction(id) => {
                &self
                    .catalog
                    .callables
                    .get(&CallableTemplateOrigin::GenericFunction(id.persistent()))?
                    .definition_origin
            }
            ImportedTarget::EnumVariant(id) => {
                &self
                    .catalog
                    .callables
                    .get(&CallableTemplateOrigin::VariantConstructor(id.persistent()))?
                    .definition_origin
            }
            ImportedTarget::Property(id) => {
                &self
                    .catalog
                    .properties
                    .get(&PropertyOwner::Property(id.persistent()))?
                    .definition_origin
            }
            ImportedTarget::ExtensionProperty(id) => {
                &self
                    .catalog
                    .properties
                    .get(&PropertyOwner::ExtensionProperty(id.persistent()))?
                    .definition_origin
            }
            ImportedTarget::Annotation(id) => {
                &self
                    .catalog
                    .annotations
                    .get(&id.persistent())?
                    .declaration
                    .definition_origin
            }
            ImportedTarget::TypeAlias(id) => self
                .catalog
                .type_aliases
                .get(&id.persistent())?
                .interface
                .definition_origin(),
        };
        Some(origin.origin())
    }
}
