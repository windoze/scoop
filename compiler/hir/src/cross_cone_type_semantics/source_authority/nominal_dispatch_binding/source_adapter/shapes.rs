use super::*;

impl NominalSourceShapeSemanticAuthority<Error>
    for BoundNominalDispatchSourcesV1<'_, '_, '_, '_, '_>
{
    fn struct_field_key(
        &mut self,
        id: PersistentFieldId,
    ) -> Result<Cow<'_, FieldIdentityKey>, Error> {
        self.parameters
            .members()
            .nominals
            .struct_field_key(id)
            .map(Cow::Borrowed)
            .map_err(Into::into)
    }
    fn enum_variant_key(
        &mut self,
        id: PersistentEnumVariantId,
    ) -> Result<Cow<'_, EnumVariantIdentityKey>, Error> {
        self.parameters
            .members()
            .nominals
            .enum_variant_key(id)
            .map(Cow::Borrowed)
            .map_err(Into::into)
    }
    fn enum_variant_field_key(
        &mut self,
        id: PersistentEnumVariantFieldId,
    ) -> Result<Cow<'_, EnumVariantFieldKey>, Error> {
        self.parameters
            .members()
            .nominals
            .enum_variant_field_key(id)
            .map(Cow::Borrowed)
            .map_err(Into::into)
    }
    fn object_value_key(
        &mut self,
        id: PersistentObjectValueId,
    ) -> Result<Cow<'_, SourceDeclarationKey>, Error> {
        self.parameters
            .members()
            .nominals
            .object_value_key(id)
            .map(Cow::Borrowed)
            .map_err(Into::into)
    }
}
