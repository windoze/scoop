use super::*;

impl BoundNominalSourceContractsV1<'_, '_> {
    /// Resolves a source struct's declaration-order position from the bound
    /// artifact, without granting field access or physical layout authority.
    pub fn struct_field_index(
        &self,
        owner: SourceNominalId,
        field: PersistentFieldId,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<u32, NominalSourceBindingError> {
        meter.charge_work(
            u64::from(self.table.records().len().max(1).ilog2()) + 1,
            path,
        )?;
        let source = self.nominal_source(owner)?;
        let NominalSourceShapeV1::Struct(shape) = source.source_shape() else {
            return Err(Error::FieldOwner { owner, field });
        };
        meter.charge_work(u64::from(self.fields.len().max(1).ilog2()) + 1, path)?;
        if self.nominal_field_key(field)?.source_owner() != Some(owner) {
            return Err(Error::FieldOwner { owner, field });
        }
        meter.check_table_entries(shape.fields().len() as u64, path)?;
        meter.charge_work(
            (shape.fields().len() as u64)
                .saturating_mul(64)
                .saturating_add(1),
            path,
        )?;
        // Canonical source fields preserve declaration order, not ID order.
        shape
            .fields()
            .iter()
            .position(|source| source.field() == field)
            .map(|index| index as u32)
            .ok_or(Error::FieldOwner { owner, field })
    }
}
