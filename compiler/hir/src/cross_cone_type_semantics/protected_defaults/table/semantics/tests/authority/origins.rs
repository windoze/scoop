use super::*;

impl ExportDefinitionSourceSemanticAuthority<&'static str> for Authority<'_> {
    fn current_cone(&self) -> ConeIdentity {
        self.fixture.origin.origin().source().cone()
    }
    fn validate_export_definition_source(
        &mut self,
        origin: &ExportDefinitionSourceV1,
    ) -> Result<(), &'static str> {
        if origin == &self.fixture.origin {
            Ok(())
        } else {
            Err("unknown definition or evaluation origin")
        }
    }
}
impl ProtectedDefaultOriginSemanticAuthority<&'static str> for Authority<'_> {
    fn validate_protected_default_origin(
        &mut self,
        key: ProtectedDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        origin: &ExportDefinitionSourceV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), &'static str> {
        self.check_source(key, root, path, meter)?;
        self.origin_calls += 1;
        if self.case != Case::Origin && origin == &self.fixture.origin {
            Ok(())
        } else {
            Err("wrong default source subject")
        }
    }
    fn validate_protected_default_local_origin(
        &mut self,
        key: ProtectedDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        local: &LocalValueSelector,
        origin: &ExportDefinitionSourceV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), &'static str> {
        self.check_source(key, root, path, meter)?;
        self.origin_calls += 1;
        let declared = self.case.flow() && local == &declared_local();
        if origin == &self.fixture.origin
            && (local == &LocalValueSelector::This || local == &parameter() || declared)
        {
            Ok(())
        } else {
            Err("unknown local source subject")
        }
    }
}
