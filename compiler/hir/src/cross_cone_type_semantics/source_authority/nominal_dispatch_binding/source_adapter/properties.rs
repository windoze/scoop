use super::*;

impl ProtectedPropertySemanticAuthority<Error>
    for BoundNominalDispatchSourcesV1<'_, '_, '_, '_, '_>
{
    fn property_source_key(
        &self,
        id: PersistentPropertyId,
    ) -> Result<&SourceDeclarationKey, Error> {
        self.parameters
            .members()
            .property_source_key(id)
            .map_err(Into::into)
    }
    fn property_source_shape(
        &self,
        id: PersistentPropertyId,
    ) -> Result<ProtectedPropertySourceShapeV1, Error> {
        self.parameters
            .members()
            .property_source_shape(id)
            .map_err(Into::into)
    }
}
impl NominalSupportPropertySemanticAuthority<Error>
    for BoundNominalDispatchSourcesV1<'_, '_, '_, '_, '_>
{
    fn const_source(
        &self,
        id: PersistentPropertyId,
    ) -> Result<&ConstPropertyDeclarationSourceV1, Error> {
        self.parameters
            .members()
            .const_source(id)
            .map_err(Into::into)
    }
    fn canonical_const_value_type(
        &self,
        kind: CanonicalConstValueKindV1,
    ) -> Result<PersistentTypeId, Error> {
        self.parameters
            .members()
            .canonical_const_value_type(kind)
            .map_err(Into::into)
    }
}
