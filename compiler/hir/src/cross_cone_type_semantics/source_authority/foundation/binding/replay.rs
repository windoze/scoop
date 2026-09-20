//! Local replay adapters. Dependency composition and the complete type-section
//! transaction supply the remaining representation/public/selection proofs.

use super::*;

impl NominalInheritanceSemanticAuthority<TypeFoundationBindingError>
    for BoundTypeFoundationSourcesV1<'_>
{
    fn exact_type_key(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&ExactTypeKey, TypeFoundationBindingError> {
        BoundTypeFoundationSourcesV1::exact_type_key(self, exact)
    }

    fn nominal_declaration_key(
        &self,
        owner: SourceNominalId,
    ) -> Result<&SourceDeclarationKey, TypeFoundationBindingError> {
        self.nominal_key(owner)
    }

    fn nominal_access_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&DeclarationAccessSourceV1, TypeFoundationBindingError> {
        self.nominal_source(owner).map(TypeSourceNominalV1::access)
    }

    fn nominal_definition_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, TypeFoundationBindingError> {
        self.nominal_source(owner)
            .map(|source| source.access().definition_origin())
    }

    fn validate_definition_source(
        &self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), TypeFoundationBindingError> {
        if self.contains_definition_source(source) {
            Ok(())
        } else {
            Err(TypeFoundationBindingError::MissingDefinitionSource)
        }
    }

    fn object_representation(
        &self,
        owner: PersistentTypeId,
    ) -> Result<&NominalRepresentationSupportV1, TypeFoundationBindingError> {
        self.source
            .entries()
            .representations
            .get(owner)
            .filter(|record| matches!(record.shape(), NominalRepresentationShapeV1::Object { .. }))
            .ok_or(TypeFoundationBindingError::MissingObject(owner))
    }

    fn generated_nominal_key(
        &self,
        owner: PersistentTypeId,
    ) -> Result<&GeneratedNominalKey, TypeFoundationBindingError> {
        self.generated_key(owner)
    }
}

impl ExactTypeFactsSemanticAuthority<TypeFoundationBindingError>
    for BoundTypeFoundationSourcesV1<'_>
{
    fn fact_shape(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&ExactTypeFactShapeV1, TypeFoundationBindingError> {
        self.source
            .entries()
            .fact_shapes
            .get(exact)
            .ok_or(TypeFoundationBindingError::MissingFactShape(exact))
    }
}
