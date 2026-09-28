use super::BodyProjection;

impl BodyProjection<'_, '_> {
    pub(super) fn generic_delegate_reference(
        &self,
        reference: &crate::GenericDelegateReference,
    ) -> Result<crate::DefaultGenericDelegateReferenceV1, super::super::DefaultBodyProjectionError>
    {
        let export = self.entities.export();
        let property = export.generic_delegate_templates[reference.template].property;
        let crate::HirPropertyIdentity::Extension(identity) = &export.property_identities[property]
        else {
            unreachable!("a generic delegate has an extension property identity")
        };
        let arguments = reference
            .arguments
            .iter()
            .map(|argument| self.type_key(*argument))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(crate::DefaultGenericDelegateReferenceV1::new(
            identity.id(),
            scoop_identity::NonEmptyVec::new(arguments).expect("source arguments are non-empty"),
        ))
    }
}
