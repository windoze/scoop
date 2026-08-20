use super::*;

pub(super) fn collect(
    inputs: &HirExportBindingIdentityInputs<'_>,
    records: &mut Vec<HirExportBindingIdentity>,
) -> Result<(), HirExportBindingIdentityError> {
    for &id in &inputs.surface.enums {
        let source = inputs.nominal_identities[id]
            .source()
            .expect("the public enum binding already has a source declaration");
        let declaration = source.declaration();
        let enumeration = raw_index(id);
        for index in 0..inputs.enums[id].variants.len() {
            let index = u32::try_from(index)
                .map_err(|_| HirExportBindingIdentityError::TooManyEnumVariants { enumeration })?;
            let identity = crate::EnumVariantRef::checked(inputs.enums, id, index)
                .and_then(|reference| inputs.enum_member_identities.get_variant(reference))
                .ok_or(HirExportBindingIdentityError::UnknownEnumVariant {
                    enumeration,
                    variant: index,
                })?;
            let name = identity.key().source_name().ok_or(
                HirExportBindingIdentityError::NonSourceEntity {
                    kind: HirExportBindingEntityKind::EnumVariant,
                    index,
                },
            )?;
            let key = ExportBindingKey::new(
                declaration.origin(),
                declaration.package().clone(),
                name.clone(),
                BindingTarget::enum_variant(identity.id()),
            );
            records.push(CborIdentityRecord::from_key(key).map_err(|error| {
                HirExportBindingIdentityError::InvalidIdentity {
                    kind: HirExportBindingEntityKind::EnumVariant,
                    index,
                    error,
                }
            })?);
        }
    }
    Ok(())
}
