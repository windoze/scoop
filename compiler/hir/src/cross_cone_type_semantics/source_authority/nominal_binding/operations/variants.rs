use super::*;

impl BoundNominalSourceContractsV1<'_, '_> {
    pub(super) fn operation_variant(
        &self,
        applied: &Applied<'_, '_>,
        variant: PersistentEnumVariantId,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<&EnumSourceVariantV1, Error> {
        applied.require_kind(PublicNominalKindV1::Enum)?;
        query(self.variants.len(), meter, path)?;
        let key = self.enum_variant_key(variant)?;
        if key.source_owner() != Some(applied.source.owner()) {
            return Err(Error::VariantOwner {
                variant,
                owner: applied.source.owner(),
            });
        }
        query(self.variant_sources.len(), meter, path)?;
        self.enum_variant_shape(variant).map_err(Into::into)
    }
    pub(super) fn operation_variant_field(
        &self,
        reference: &DefaultEnumVariantFieldRefV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultVariantFieldOperationShapeV1, Error> {
        let applied = Applied::new(self, reference.owner_type(), meter, path)?;
        query(self.variant_fields.len(), meter, path)?;
        let key = self.enum_variant_field_key(reference.declaration())?;
        let variant = self.operation_variant(&applied, key.variant(), meter, path)?;
        meter.check_table_entries(variant.fields().len() as u64, path)?;
        meter.charge_work((variant.fields().len() as u64).saturating_mul(32), path)?;
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
            applied.owner_type(meter, path)?,
            index,
            applied.field_type(field.value_type(), meter, path)?,
        ))
    }
}
