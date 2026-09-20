use super::*;
use scoop_identity::{
    EnumVariantFieldKey, EnumVariantIdentityKey, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentGenericTypeId, PersistentPropertyAccessorId,
    PersistentPropertyId, PersistentTypeId, PropertyAccessorKey, SignatureTypeKey,
};

impl ProtectedCallableSemanticAuthority<Error>
    for BoundNominalDispatchSourcesV1<'_, '_, '_, '_, '_>
{
    fn callable_source_key(
        &self,
        declaration: CallableTemplateOrigin,
    ) -> Result<&SourceDeclarationKey, Error> {
        match declaration {
            CallableTemplateOrigin::Constructor(id) => self
                .parameters
                .constructors()
                .constructor_key(id)
                .map_err(Into::into),
            _ => self
                .parameters
                .members()
                .callable_key(declaration)
                .map_err(Into::into),
        }
    }
    fn property_accessor_key(
        &self,
        accessor: PersistentPropertyAccessorId,
    ) -> Result<&PropertyAccessorKey, Error> {
        self.parameters
            .members()
            .property_accessor_key(accessor)
            .map_err(Into::into)
    }
    fn property_value_type(
        &self,
        property: PersistentPropertyId,
    ) -> Result<&SignatureTypeKey, Error> {
        self.parameters
            .members()
            .property_value_type(property)
            .map_err(Into::into)
    }
    fn unit_type(&self) -> Result<PersistentTypeId, Error> {
        self.parameters.members().unit_type().map_err(Into::into)
    }
}
impl NominalInterfaceShapeAuthority<Error> for BoundNominalDispatchSourcesV1<'_, '_, '_, '_, '_> {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, Error> {
        self.parameters
            .members()
            .nominals
            .signature_nominal_shape(SourceNominalId::Concrete(declaration))
            .map_err(Into::into)
    }
    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, Error> {
        self.parameters
            .members()
            .nominals
            .signature_nominal_shape(SourceNominalId::GenericTemplate(declaration))
            .map_err(Into::into)
    }
}
impl NominalSupportCallableSemanticAuthority<Error>
    for BoundNominalDispatchSourcesV1<'_, '_, '_, '_, '_>
{
    fn source_nominal_modality(
        &self,
        owner: SourceNominalId,
    ) -> Result<NominalInheritanceModalityV1, Error> {
        self.parameters
            .members()
            .nominals
            .nominal_source(owner)
            .map(NominalSourceContractV1::modality)
            .map_err(Into::into)
    }
    fn source_enum_variant_key(
        &self,
        variant: PersistentEnumVariantId,
    ) -> Result<&EnumVariantIdentityKey, Error> {
        self.parameters
            .members()
            .nominals
            .enum_variant_key(variant)
            .map_err(Into::into)
    }
    fn source_enum_variant_shape(
        &self,
        variant: PersistentEnumVariantId,
    ) -> Result<&EnumSourceVariantV1, Error> {
        self.parameters
            .members()
            .nominals
            .enum_variant_shape(variant)
            .map_err(Into::into)
    }
    fn source_enum_variant_field_key(
        &self,
        field: PersistentEnumVariantFieldId,
    ) -> Result<&EnumVariantFieldKey, Error> {
        self.parameters
            .members()
            .nominals
            .enum_variant_field_key(field)
            .map_err(Into::into)
    }
    fn source_enum_variant_origin(
        &self,
        variant: PersistentEnumVariantId,
    ) -> Result<&ExportDefinitionSourceV1, Error> {
        self.parameters
            .members()
            .nominals
            .enum_variant_origin(variant)
            .map_err(Into::into)
    }
}
