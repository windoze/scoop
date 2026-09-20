use super::*;
use scoop_identity::{ExactTypeKey, GeneratedNominalKey, PersistentTypeId};

impl NominalInheritanceSemanticAuthority<Error> for BoundInheritanceSourcesV1<'_, '_, '_> {
    fn exact_type_key(&self, exact: PersistentExactTypeId) -> Result<&ExactTypeKey, Error> {
        self.protected
            .foundation
            .exact_type_key(exact)
            .map_err(Into::into)
    }
    fn nominal_declaration_key(
        &self,
        owner: SourceNominalId,
    ) -> Result<&SourceDeclarationKey, Error> {
        self.protected
            .foundation
            .nominal_key(owner)
            .map_err(Into::into)
    }
    fn nominal_access_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&DeclarationAccessSourceV1, Error> {
        self.protected
            .foundation
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
        self.protected
            .foundation
            .validate_definition_source(source)
            .map_err(Into::into)
    }
    fn object_representation(
        &self,
        owner: PersistentTypeId,
    ) -> Result<&NominalRepresentationSupportV1, Error> {
        self.protected
            .foundation
            .object_representation(owner)
            .map_err(Into::into)
    }
    fn generated_nominal_key(
        &self,
        owner: PersistentTypeId,
    ) -> Result<&GeneratedNominalKey, Error> {
        self.protected
            .foundation
            .generated_key(owner)
            .map_err(Into::into)
    }
}
