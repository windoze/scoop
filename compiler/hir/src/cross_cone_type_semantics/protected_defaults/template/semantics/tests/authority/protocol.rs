use super::*;

pub(in super::super) struct ProtocolAuthority {
    owner: CallableTemplateOrigin,
    shapes: Vec<SourceParameterShapeV1>,
    origin: ExportDefinitionSourceV1,
}
impl ProtocolAuthority {
    pub fn new(case: &Case) -> Self {
        Self {
            owner: case.key.owner(),
            shapes: case.record.payload().parameters().parameters().to_vec(),
            origin: case.origin.clone(),
        }
    }
}
impl ProtectedSourceProtocolSemanticAuthority<&'static str> for ProtocolAuthority {
    fn canonical_array_type(&mut self) -> Result<PersistentGenericTypeId, &'static str> {
        Err("no array parameter")
    }
    fn source_parameter_shape(
        &self,
        owner: CallableTemplateOrigin,
        position: u32,
    ) -> Result<&SourceParameterShapeV1, &'static str> {
        if owner != self.owner {
            return Err("wrong source owner");
        }
        self.shapes
            .get(position as usize)
            .ok_or("wrong source parameter")
    }
    fn source_parameter_calling_kind(
        &self,
        owner: CallableTemplateOrigin,
        position: u32,
    ) -> Result<ProtectedParameterCallingKindV1, &'static str> {
        if owner != self.owner || position as usize >= self.shapes.len() {
            return Err("wrong source parameter");
        }
        Ok(if position as usize + 1 == self.shapes.len() {
            ProtectedParameterCallingKindV1::Default
        } else {
            ProtectedParameterCallingKindV1::Required
        })
    }
    fn validate_source_parameter_origin(
        &self,
        owner: CallableTemplateOrigin,
        position: u32,
        origin: &ExportDefinitionSourceV1,
    ) -> Result<(), &'static str> {
        if owner == self.owner && (position as usize) < self.shapes.len() && origin == &self.origin
        {
            Ok(())
        } else {
            Err("wrong source parameter origin")
        }
    }
}
