use super::*;

pub(super) struct Authority {
    owner: CallableTemplateOrigin,
    parameters: Vec<SourceParameterShapeV1>,
    origin: ExportDefinitionSourceV1,
}
impl Authority {
    pub fn new(fixture: &Fixture) -> Self {
        Self {
            owner: fixture.key.owner(),
            parameters: (0..2)
                .map(|index| {
                    SourceParameterShapeV1::new(
                        CanonicalIdentifier::new(&format!("p{index}")).unwrap(),
                        fixture.unit.clone(),
                    )
                })
                .collect(),
            origin: fixture.origin.clone(),
        }
    }
}
impl ProtectedSourceProtocolSemanticAuthority<&'static str> for Authority {
    fn canonical_array_type(&mut self) -> Result<PersistentGenericTypeId, &'static str> {
        Err("no vararg source")
    }
    fn source_parameter_shape(
        &self,
        owner: CallableTemplateOrigin,
        position: u32,
    ) -> Result<&SourceParameterShapeV1, &'static str> {
        if owner != self.owner {
            return Err("unknown parameter owner");
        }
        self.parameters
            .get(position as usize)
            .ok_or("unknown parameter position")
    }
    fn source_parameter_calling_kind(
        &self,
        owner: CallableTemplateOrigin,
        position: u32,
    ) -> Result<ProtectedParameterCallingKindV1, &'static str> {
        self.source_parameter_shape(owner, position)?;
        Ok(if position == 0 {
            ProtectedParameterCallingKindV1::Required
        } else {
            ProtectedParameterCallingKindV1::Default
        })
    }
    fn validate_source_parameter_origin(
        &self,
        owner: CallableTemplateOrigin,
        position: u32,
        origin: &ExportDefinitionSourceV1,
    ) -> Result<(), &'static str> {
        self.source_parameter_shape(owner, position)?;
        if origin == &self.origin {
            Ok(())
        } else {
            Err("wrong parameter origin")
        }
    }
}
