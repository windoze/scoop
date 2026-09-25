use super::*;

impl BoundNominalSourceContractsV1<'_, '_> {
    pub(super) fn operation_variant(
        &self,
        applied: &Applied<'_, '_>,
        variant: PersistentEnumVariantId,
    ) -> Result<&EnumSourceVariantV1, Error> {
        applied.require_kind(PublicNominalKindV1::Enum)?;

        let key = self.enum_variant_key(variant)?;
        if key.source_owner() != Some(applied.source.owner()) {
            return Err(Error::VariantOwner {
                variant,
                owner: applied.source.owner(),
            });
        }

        self.enum_variant_shape(variant).map_err(Into::into)
    }
    pub(super) fn operation_variant_field(
        &self,
        reference: &DefaultEnumVariantFieldRefV1,

        path: &WirePath,
    ) -> Result<DefaultVariantFieldOperationShapeV1, Error> {
        let applied = Applied::new(self, reference.owner_type(), path)?;

        let key = self.enum_variant_field_key(reference.declaration())?;
        let variant = self.operation_variant(&applied, key.variant())?;

        let (index, field) = variant
            .fields()
            .iter()
            .enumerate()
            .find(|(_, field)| field.field() == reference.declaration())
            .ok_or_else(|| {
                NominalSourceBindingError::MissingVariantField(reference.declaration())
            })?;
        let index = u32::try_from(index).map_err(|_| Error::FieldPosition)?;
        Ok(DefaultVariantFieldOperationShapeV1::new(
            applied.owner_type(path)?,
            index,
            applied.field_type(field.value_type())?,
        ))
    }
}
