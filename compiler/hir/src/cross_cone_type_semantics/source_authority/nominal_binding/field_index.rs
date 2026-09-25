use super::*;

impl BoundNominalSourceContractsV1<'_, '_> {
    /// Resolves a source struct's declaration-order position from the bound
    /// artifact, without granting field access or physical layout authority.
    pub fn struct_field_index(
        &self,
        owner: SourceNominalId,
        field: PersistentFieldId,
    ) -> Result<u32, NominalSourceBindingError> {
        let source = self.nominal_source(owner)?;
        let NominalSourceShapeV1::Struct(shape) = source.source_shape() else {
            return Err(Error::FieldOwner { owner, field });
        };

        if self.nominal_field_key(field)?.source_owner() != Some(owner) {
            return Err(Error::FieldOwner { owner, field });
        }

        // Canonical source fields preserve declaration order, not ID order.
        shape
            .fields()
            .iter()
            .position(|source| source.field() == field)
            .map(|index| index as u32)
            .ok_or(Error::FieldOwner { owner, field })
    }
}
