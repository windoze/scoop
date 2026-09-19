//! The complete eight-field type-semantics transport. Publication and selected
//! capabilities are obtained only through the section's semantic validator.
use crate::*;

mod decode;
mod indexed;
#[cfg(test)]
mod tests;
pub use decode::*;
pub use indexed::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossConeTypeSemanticsSectionV1 {
    exact_facts: CanonicalExactTypeFactsV1,
    representation_support: CanonicalNominalRepresentationSupportV1,
    inheritance: CanonicalNominalInheritanceInterfacesV1,
    protected_declarations: CanonicalProtectedDeclarationInterfacesV1,
    protected_source_interfaces: CanonicalProtectedCallableSourceInterfacesV1,
    protected_defaults: CanonicalProtectedDefaultTemplatesV1,
    definition_sources: CanonicalExportDefinitionSourcesV1,
    selected: CanonicalSelectedExternalTypeUsesV1,
}
impl CrossConeTypeSemanticsSectionV1 {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        exact_facts: CanonicalExactTypeFactsV1,
        representation_support: CanonicalNominalRepresentationSupportV1,
        inheritance: CanonicalNominalInheritanceInterfacesV1,
        protected_declarations: CanonicalProtectedDeclarationInterfacesV1,
        protected_source_interfaces: CanonicalProtectedCallableSourceInterfacesV1,
        protected_defaults: CanonicalProtectedDefaultTemplatesV1,
        definition_sources: CanonicalExportDefinitionSourcesV1,
        selected: CanonicalSelectedExternalTypeUsesV1,
    ) -> Self {
        Self {
            exact_facts,
            representation_support,
            inheritance,
            protected_declarations,
            protected_source_interfaces,
            protected_defaults,
            definition_sources,
            selected,
        }
    }
    pub const fn exact_facts(&self) -> &CanonicalExactTypeFactsV1 {
        &self.exact_facts
    }
    pub const fn representation_support(&self) -> &CanonicalNominalRepresentationSupportV1 {
        &self.representation_support
    }
    pub const fn inheritance(&self) -> &CanonicalNominalInheritanceInterfacesV1 {
        &self.inheritance
    }
    pub const fn protected_declarations(&self) -> &CanonicalProtectedDeclarationInterfacesV1 {
        &self.protected_declarations
    }
    pub const fn protected_source_interfaces(
        &self,
    ) -> &CanonicalProtectedCallableSourceInterfacesV1 {
        &self.protected_source_interfaces
    }
    pub const fn protected_defaults(&self) -> &CanonicalProtectedDefaultTemplatesV1 {
        &self.protected_defaults
    }
    pub const fn definition_sources(&self) -> &CanonicalExportDefinitionSourcesV1 {
        &self.definition_sources
    }
    pub const fn selected(&self) -> &CanonicalSelectedExternalTypeUsesV1 {
        &self.selected
    }

    pub fn definition_source_inputs(&self) -> TypeDefinitionSourceInputsV1<'_> {
        TypeDefinitionSourceInputsV1 {
            representations: &self.representation_support,
            inheritance: &self.inheritance,
            protected_declarations: &self.protected_declarations,
            source_interfaces: &self.protected_source_interfaces,
            defaults: &self.protected_defaults,
        }
    }
}
