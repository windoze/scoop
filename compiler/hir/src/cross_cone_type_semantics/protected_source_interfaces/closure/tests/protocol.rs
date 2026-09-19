use super::*;
pub(super) struct Authority;
impl ProtectedSourceProtocolSemanticAuthority<&'static str> for Authority {
    fn canonical_array_type(&mut self) -> Result<PersistentGenericTypeId, &'static str> {
        Err("no vararg declaration")
    }
    fn source_parameter_shape(
        &self,
        _: CallableTemplateOrigin,
        _: u32,
    ) -> Result<&SourceParameterShapeV1, &'static str> {
        Err("no source parameters")
    }
    fn source_parameter_calling_kind(
        &self,
        _: CallableTemplateOrigin,
        _: u32,
    ) -> Result<ProtectedParameterCallingKindV1, &'static str> {
        Err("no source parameters")
    }
    fn validate_source_parameter_origin(
        &self,
        _: CallableTemplateOrigin,
        _: u32,
        _: &ExportDefinitionSourceV1,
    ) -> Result<(), &'static str> {
        Err("no source parameters")
    }
}
