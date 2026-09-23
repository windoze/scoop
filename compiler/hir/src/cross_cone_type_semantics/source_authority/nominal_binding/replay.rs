use super::*;
use scoop_identity::{PersistentGenericTypeId, PersistentTypeId, SourceDeclarationKind};

impl BoundNominalSourceContractsV1<'_, '_> {
    pub(in crate::cross_cone_type_semantics::source_authority) fn signature_nominal_shape(
        &self,
        owner: SourceNominalId,
    ) -> Result<PublicNominalShapeV1, Error> {
        let key = match owner {
            SourceNominalId::Concrete(id) => self
                .foundation
                .identities
                .canonical_key::<PersistentTypeId, SourceDeclarationKey>(id),
            SourceNominalId::GenericTemplate(id) => {
                self.foundation
                    .identities
                    .canonical_key::<PersistentGenericTypeId, SourceDeclarationKey>(id)
            }
        }
        .map_err(|error| Error::Identity(error.to_string()))?;
        shape(&key)
    }
}
impl NominalInterfaceShapeAuthority<Error> for BoundNominalSourceContractsV1<'_, '_> {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, Error> {
        self.signature_nominal_shape(SourceNominalId::Concrete(declaration))
    }
    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, Error> {
        self.signature_nominal_shape(SourceNominalId::GenericTemplate(declaration))
    }
}

pub(super) fn shape(key: &SourceDeclarationKey) -> Result<PublicNominalShapeV1, Error> {
    let kind = match key.declaration_kind() {
        SourceDeclarationKind::Class => PublicNominalKindV1::Class,
        SourceDeclarationKind::Struct => PublicNominalKindV1::Struct,
        SourceDeclarationKind::Enum => PublicNominalKindV1::Enum,
        SourceDeclarationKind::Interface => PublicNominalKindV1::Interface,
        SourceDeclarationKind::Object => PublicNominalKindV1::Object,
        _ => return Err(Error::Identity("expected a source nominal key".into())),
    };
    Ok(PublicNominalShapeV1::new(
        kind,
        key.duplicate_signature().type_parameter_count(),
    ))
}

impl NominalSourceShapeSemanticAuthority<Error> for BoundNominalSourceContractsV1<'_, '_> {
    fn nominal_field_key(
        &mut self,
        field: PersistentFieldId,
    ) -> Result<std::borrow::Cow<'_, FieldIdentityKey>, Error> {
        BoundNominalSourceContractsV1::nominal_field_key(self, field)
            .map(std::borrow::Cow::Borrowed)
    }
    fn enum_variant_key(
        &mut self,
        variant: PersistentEnumVariantId,
    ) -> Result<std::borrow::Cow<'_, EnumVariantIdentityKey>, Error> {
        BoundNominalSourceContractsV1::enum_variant_key(self, variant)
            .map(std::borrow::Cow::Borrowed)
    }
    fn enum_variant_field_key(
        &mut self,
        field: PersistentEnumVariantFieldId,
    ) -> Result<std::borrow::Cow<'_, EnumVariantFieldKey>, Error> {
        BoundNominalSourceContractsV1::enum_variant_field_key(self, field)
            .map(std::borrow::Cow::Borrowed)
    }
    fn object_value_key(
        &mut self,
        value: PersistentObjectValueId,
    ) -> Result<std::borrow::Cow<'_, SourceDeclarationKey>, Error> {
        BoundNominalSourceContractsV1::object_value_key(self, value).map(std::borrow::Cow::Borrowed)
    }
}
