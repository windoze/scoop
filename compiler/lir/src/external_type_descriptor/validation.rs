use super::*;

impl ExternalTypeDescriptor {
    pub fn validate_definition(
        self,
        definitions: &crate::StrongObjectSymbolSurfaceV1,
    ) -> Result<(), ExternalTypeDescriptorValidationError> {
        self.validate_contract()?;
        let plan = definitions.plan(self.required_definition).ok_or(
            ExternalTypeDescriptorValidationError::MissingDefinition(self.required_definition),
        )?;
        if plan.owner() != StrongDefinitionEntity::exact_type(self.target)
            || plan.definition_role() != StrongDefinitionRole::TypeDescriptor
            || plan.primary_symbol() != self.expected_symbol
        {
            return Err(ExternalTypeDescriptorValidationError::DefinitionMismatch(
                self.required_definition,
            ));
        }
        Ok(())
    }

    pub(crate) fn validate_contract(self) -> Result<(), ExternalTypeDescriptorValidationError> {
        let expected = Self::new(self.provider, self.target)
            .map_err(ExternalTypeDescriptorValidationError::Build)?;
        if self != expected {
            return Err(ExternalTypeDescriptorValidationError::ContractMismatch);
        }
        Ok(())
    }
}
