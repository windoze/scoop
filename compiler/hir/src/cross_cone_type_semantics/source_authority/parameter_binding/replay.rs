use super::*;

impl ProtectedSourceProtocolSemanticAuthority<InheritanceParameterBindingError>
    for BoundInheritanceParameterProtocolsV1<'_>
{
    fn canonical_array_type(
        &mut self,
    ) -> Result<PersistentGenericTypeId, InheritanceParameterBindingError> {
        Ok(self.array)
    }
    fn source_parameter_shape(
        &self,
        owner: CallableTemplateOrigin,
        position: u32,
    ) -> Result<&SourceParameterShapeV1, InheritanceParameterBindingError> {
        self.parameter(owner, position)
            .map(InheritanceSourceParameterV1::shape)
    }
    fn source_parameter_calling_kind(
        &self,
        owner: CallableTemplateOrigin,
        position: u32,
    ) -> Result<ProtectedParameterCallingKindV1, InheritanceParameterBindingError> {
        self.parameter(owner, position)
            .map(InheritanceSourceParameterV1::calling_kind)
    }
    fn validate_source_parameter_origin(
        &self,
        owner: CallableTemplateOrigin,
        position: u32,
        origin: &ExportDefinitionSourceV1,
    ) -> Result<(), InheritanceParameterBindingError> {
        if self.parameter(owner, position)?.definition_origin() != origin {
            return Err(InheritanceParameterBindingError::Origin { owner, position });
        }
        Ok(())
    }
}
