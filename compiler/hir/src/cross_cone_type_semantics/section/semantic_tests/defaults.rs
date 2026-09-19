use super::*;
mod access;
mod body;
mod origins;
mod protocols;

pub(super) struct DefaultAuthority {
    provider: ConeIdentity,
    origins: Vec<ExportDefinitionSourceV1>,
}
impl DefaultAuthority {
    pub(super) fn new(fixture: &Fixture) -> Self {
        Self {
            provider: fixture.provider,
            origins: fixture.source.graph.origins.values().cloned().collect(),
        }
    }
}
impl ExportDefinitionSourceSemanticAuthority<&'static str> for DefaultAuthority {
    fn current_cone(&self) -> ConeIdentity {
        self.provider
    }
    fn validate_export_definition_source(
        &mut self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), &'static str> {
        if self.origins.contains(source) {
            Ok(())
        } else {
            Err("foreign source origin")
        }
    }
}
impl TypeDefinitionSourceSemanticAuthority<&'static str> for DefaultAuthority {
    fn validate_type_definition_source_use(
        &mut self,
        _source_use: TypeDefinitionSourceUseV1<'_>,
        source: &ExportDefinitionSourceV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), &'static str> {
        meter.charge_work(1, path).map_err(|_| "origin budget")?;
        self.validate_export_definition_source(source)
    }
}
impl NominalInterfaceShapeAuthority<&'static str> for DefaultAuthority {
    fn concrete_nominal_shape(
        &mut self,
        _id: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, &'static str> {
        Err("no default shape")
    }
    fn generic_nominal_shape(
        &mut self,
        _id: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, &'static str> {
        Err("no default shape")
    }
}
