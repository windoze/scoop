use super::*;
use scoop_identity::GeneratedNominalKey;

impl NominalInheritanceSemanticAuthority<Error>
    for BoundInheritanceProtectedCallableSourcesV1<'_, '_>
{
    fn exact_type_key(&self, exact: PersistentExactTypeId) -> Result<&ExactTypeKey, Error> {
        self.foundation.exact_type_key(exact).map_err(Into::into)
    }
    fn nominal_declaration_key(
        &self,
        owner: SourceNominalId,
    ) -> Result<&SourceDeclarationKey, Error> {
        self.foundation.nominal_key(owner).map_err(Into::into)
    }
    fn nominal_access_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&DeclarationAccessSourceV1, Error> {
        self.foundation
            .nominal_source(owner)
            .map(TypeSourceNominalV1::access)
            .map_err(Into::into)
    }
    fn nominal_definition_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, Error> {
        self.nominal_access_source(owner)
            .map(DeclarationAccessSourceV1::definition_origin)
    }
    fn validate_definition_source(&self, source: &ExportDefinitionSourceV1) -> Result<(), Error> {
        NominalInheritanceSemanticAuthority::validate_definition_source(self.foundation, source)
            .map_err(Into::into)
    }
    fn object_representation(
        &self,
        owner: PersistentTypeId,
    ) -> Result<&NominalRepresentationSupportV1, Error> {
        self.foundation
            .object_representation(owner)
            .map_err(Into::into)
    }
    fn generated_nominal_key(
        &self,
        owner: PersistentTypeId,
    ) -> Result<&GeneratedNominalKey, Error> {
        self.foundation.generated_key(owner).map_err(Into::into)
    }
}
