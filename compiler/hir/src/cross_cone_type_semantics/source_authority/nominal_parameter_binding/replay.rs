use super::*;

impl ProtectedSourceProtocolSemanticAuthority<NominalParameterBindingError>
    for BoundNominalParameterProtocolsV1<'_, '_, '_, '_>
{
    fn canonical_array_type(
        &mut self,
    ) -> Result<PersistentGenericTypeId, NominalParameterBindingError> {
        Ok(self.array)
    }
    fn source_parameter_shape(
        &self,
        owner: CallableTemplateOrigin,
        position: u32,
    ) -> Result<&SourceParameterShapeV1, NominalParameterBindingError> {
        self.parameter(owner, position)
            .map(InheritanceSourceParameterV1::shape)
    }
    fn source_parameter_calling_kind(
        &self,
        owner: CallableTemplateOrigin,
        position: u32,
    ) -> Result<ProtectedParameterCallingKindV1, NominalParameterBindingError> {
        self.parameter(owner, position)
            .map(InheritanceSourceParameterV1::calling_kind)
    }
    fn validate_source_parameter_origin(
        &self,
        owner: CallableTemplateOrigin,
        position: u32,
        origin: &ExportDefinitionSourceV1,
    ) -> Result<(), NominalParameterBindingError> {
        if self.parameter(owner, position)?.definition_origin() != origin {
            return Err(SourceParameterContractError::Origin { owner, position }.into());
        }
        Ok(())
    }
}
