use super::*;
use scoop_identity::{
    EnumVariantFieldKey, EnumVariantIdentityKey, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentGenericTypeId,
};

impl NominalInterfaceShapeAuthority<Error> for BoundNominalMemberSourcesV1<'_, '_, '_> {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, Error> {
        self.nominals
            .signature_nominal_shape(SourceNominalId::Concrete(declaration))
            .map_err(Into::into)
    }
    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, Error> {
        self.nominals
            .signature_nominal_shape(SourceNominalId::GenericTemplate(declaration))
            .map_err(Into::into)
    }
}
impl NominalSupportCallableSemanticAuthority<Error> for BoundNominalMemberSourcesV1<'_, '_, '_> {
    fn source_nominal_modality(
        &self,
        owner: SourceNominalId,
    ) -> Result<NominalInheritanceModalityV1, Error> {
        self.nominals
            .nominal_source(owner)
            .map(NominalSourceContractV1::modality)
            .map_err(Into::into)
    }
    fn source_enum_variant_key(
        &self,
        variant: PersistentEnumVariantId,
    ) -> Result<&EnumVariantIdentityKey, Error> {
        self.nominals.enum_variant_key(variant).map_err(Into::into)
    }
    fn source_enum_variant_shape(
        &self,
        variant: PersistentEnumVariantId,
    ) -> Result<&EnumSourceVariantV1, Error> {
        self.nominals
            .enum_variant_shape(variant)
            .map_err(Into::into)
    }
    fn source_enum_variant_field_key(
        &self,
        field: PersistentEnumVariantFieldId,
    ) -> Result<&EnumVariantFieldKey, Error> {
        self.nominals
            .enum_variant_field_key(field)
            .map_err(Into::into)
    }
    fn source_enum_variant_origin(
        &self,
        variant: PersistentEnumVariantId,
    ) -> Result<&ExportDefinitionSourceV1, Error> {
        self.nominals
            .enum_variant_origin(variant)
            .map_err(Into::into)
    }
}
