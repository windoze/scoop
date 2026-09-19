use super::*;

impl ProtectedSourceProtocolSemanticAuthority<&'static str> for DefaultAuthority {
    fn canonical_array_type(&mut self) -> Result<PersistentGenericTypeId, &'static str> {
        Err("fixture has no default template")
    }
    fn source_parameter_shape(
        &self,
        _owner: CallableTemplateOrigin,
        _position: u32,
    ) -> Result<&SourceParameterShapeV1, &'static str> {
        Err("fixture has no default template")
    }
    fn source_parameter_calling_kind(
        &self,
        _owner: CallableTemplateOrigin,
        _position: u32,
    ) -> Result<ProtectedParameterCallingKindV1, &'static str> {
        Err("fixture has no default template")
    }
    fn validate_source_parameter_origin(
        &self,
        _owner: CallableTemplateOrigin,
        _position: u32,
        _origin: &ExportDefinitionSourceV1,
    ) -> Result<(), &'static str> {
        Err("fixture has no default template")
    }
}
