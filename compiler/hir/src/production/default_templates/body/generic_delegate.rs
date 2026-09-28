use super::BodyProjection;

impl BodyProjection<'_, '_> {
    pub(super) fn generic_delegate_reference(
        &self,
        reference: &crate::GenericDelegateReference,
    ) -> Result<crate::DefaultGenericDelegateReferenceV1, super::super::DefaultBodyProjectionError>
    {
        let export = self.entities.export();
        let property = match reference.template {
            crate::GenericDelegateTemplateSource::Defined(template) => {
                let property = export.generic_delegate_templates[template].property;
                export.property_identities[property]
                    .extension_id()
                    .expect("a generic delegate has an extension property identity")
            }
            crate::GenericDelegateTemplateSource::Imported(template) => {
                export.imported_generic_delegate_templates[template].property
            }
        };
        let arguments = reference
            .arguments
            .iter()
            .map(|argument| self.type_key(*argument))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(crate::DefaultGenericDelegateReferenceV1::new(
            property,
            scoop_identity::NonEmptyVec::new(arguments).expect("source arguments are non-empty"),
        ))
    }
}
