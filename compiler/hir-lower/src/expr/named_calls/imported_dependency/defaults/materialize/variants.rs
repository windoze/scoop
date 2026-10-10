use super::*;

impl Lowerer {
    pub(super) fn materialize_imported_variant_field(
        &mut self,
        field: &hir::DefaultEnumVariantFieldRefV1,
        context: &ImportedDefaultContext<'_>,
    ) -> Result<hir::EnumVariantFieldApplication, ImportedDefaultMaterializationError> {
        let owner = self.materialize_imported_default_type(field.owner_type(), context)?;
        let hir::Type::Enum(application) = self.types[owner] else {
            return Err(ImportedDefaultMaterializationError::Plan(
                "variant payload owner must be an enum".into(),
            ));
        };
        let template = self.enum_applications[application].template;
        let variant = match self.loaded_enum_definitions.get(&template) {
            Some(definition) => (0..definition.definition.variants.len())
                .find(|&index| definition.field_index(index, field.declaration()).is_some())
                .map(|index| definition.variant_identity(index)),
            None => self.enum_member_identities.as_ref().and_then(|identities| {
                identities
                    .field_declaration(field.declaration())
                    .map(|field| identities[field.variant()].id())
            }),
        }
        .ok_or_else(|| {
            ImportedDefaultMaterializationError::Plan(
                "variant payload field is missing from its enum".into(),
            )
        })?;
        Ok(hir::EnumVariantFieldApplication {
            variant: hir::EnumVariantApplication { owner, variant },
            field: field.declaration(),
        })
    }
}
