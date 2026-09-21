use super::*;

impl CallableAbiRecordV1 {
    pub fn validate_definition(
        &self,
        producer: ConeIdentity,
        definitions: &crate::StrongObjectSymbolSurfaceV1,
    ) -> Result<(), CallableAbiValidationError> {
        let (body, symbol, definition) = self.link_contract(producer)?;
        validate_definition_contract(body, symbol, definition, definitions)
    }

    pub fn validate_against(
        &self,
        foundation: &crate::OdrFreeLirFoundation,
        definitions: &crate::StrongObjectSymbolSurfaceV1,
    ) -> Result<(), CallableAbiValidationError> {
        let (body, symbol, definition) = self.link_contract(foundation.producer())?;
        if !foundation.contains_callable_body(body) {
            return Err(CallableAbiValidationError::MissingBody(body));
        }
        if !foundation.contains_symbol_request(symbol) {
            return Err(CallableAbiValidationError::MissingSymbol(symbol));
        }
        validate_definition_contract(body, symbol, definition, definitions)
    }
    pub(crate) fn link_contract(
        &self,
        provider: ConeIdentity,
    ) -> Result<
        (
            PersistentCallableBodyId,
            PersistentSymbolRequest,
            ObjectDefinitionPlanId,
        ),
        CallableAbiValidationError,
    > {
        let contract = derive_callable_link_contract(provider, self.target)
            .map_err(CallableAbiValidationError::Contract)?;
        if self.expected_symbol != contract.1 || self.required_definition != contract.2 {
            return Err(CallableAbiValidationError::ContractMismatch);
        }
        Ok(contract)
    }
}

fn validate_definition_contract(
    body: PersistentCallableBodyId,
    symbol: PersistentSymbolRequest,
    definition: ObjectDefinitionPlanId,
    definitions: &crate::StrongObjectSymbolSurfaceV1,
) -> Result<(), CallableAbiValidationError> {
    let plan = definitions
        .plan(definition)
        .ok_or(CallableAbiValidationError::MissingDefinition(definition))?;
    if plan.owner() != StrongDefinitionEntity::callable_body(body)
        || plan.definition_role() != StrongDefinitionRole::CallableBody
        || plan.primary_symbol() != symbol
    {
        return Err(CallableAbiValidationError::DefinitionMismatch(definition));
    }
    Ok(())
}
