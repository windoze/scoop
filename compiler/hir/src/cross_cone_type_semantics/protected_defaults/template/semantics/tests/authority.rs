use super::support::Fixture;
use super::*;

mod protocol;
pub(super) use protocol::ProtocolAuthority;

pub(super) struct Authority {
    pub fixture: Fixture,
    pub key: ProtectedDefaultTemplateKeyV1,
    pub root: PersistentLexicalRootV1,
    pub path: StructuralDefinitionPath,
    pub provider: DefaultTemplateProviderShapeV1,
    pub inherited: bool,
    pub origin: ExportDefinitionSourceV1,
    pub locals: Vec<LocalValueSelector>,
    pub reject_origin: bool,
    pub reject_local: bool,
}
impl Authority {
    pub fn new(case: &Case, template: &ProtectedDefaultTemplateV1) -> Self {
        Self {
            fixture: case.fixture.clone(),
            key: case.key,
            root: template.definition_root(),
            path: template.definition_path().clone(),
            provider: case.provider,
            inherited: true,
            origin: case.origin.clone(),
            locals: template
                .locals()
                .records()
                .iter()
                .map(|local| local.selector().clone())
                .collect(),
            reject_origin: false,
            reject_local: false,
        }
    }
}
impl NominalInterfaceShapeAuthority<&'static str> for Authority {
    fn concrete_nominal_shape(
        &mut self,
        id: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, &'static str> {
        self.fixture.concrete_nominal_shape(id)
    }
    fn generic_nominal_shape(
        &mut self,
        id: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, &'static str> {
        self.fixture.generic_nominal_shape(id)
    }
}
impl ProtectedDefaultRootSemanticAuthority<&'static str> for Authority {
    fn protected_default_provider_shape(
        &mut self,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        meter: &mut BudgetMeter,
    ) -> Result<DefaultTemplateProviderShapeV1, &'static str> {
        meter
            .charge_work(path.segments().len() as u64, &WirePath::root())
            .map_err(|_| "provider budget")?;
        if root == self.root && path == &self.path {
            Ok(self.provider)
        } else {
            Err("wrong provider")
        }
    }
    fn validate_inherited_protected_default_provider(
        &mut self,
        key: ProtectedDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        meter: &mut BudgetMeter,
    ) -> Result<(), &'static str> {
        meter
            .charge_work(1, &WirePath::root())
            .map_err(|_| "provider budget")?;
        if self.inherited && key == self.key && root == self.root && path == &self.path {
            Ok(())
        } else {
            Err("wrong inherited provider")
        }
    }
}
impl ExportDefinitionSourceSemanticAuthority<&'static str> for Authority {
    fn current_cone(&self) -> ConeIdentity {
        self.origin.origin().source().cone()
    }
    fn validate_export_definition_source(
        &mut self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), &'static str> {
        if source == &self.origin {
            Ok(())
        } else {
            Err("unknown source origin")
        }
    }
}
impl ProtectedDefaultOriginSemanticAuthority<&'static str> for Authority {
    fn validate_protected_default_origin(
        &mut self,
        key: ProtectedDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        origin: &ExportDefinitionSourceV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), &'static str> {
        meter
            .charge_work(path.segments().len() as u64, &WirePath::root())
            .map_err(|_| "origin budget")?;
        if !self.reject_origin
            && key == self.key
            && root == self.root
            && path == &self.path
            && origin == &self.origin
        {
            Ok(())
        } else {
            Err("wrong default origin")
        }
    }
    fn validate_protected_default_local_origin(
        &mut self,
        key: ProtectedDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        selector: &LocalValueSelector,
        origin: &ExportDefinitionSourceV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), &'static str> {
        meter
            .charge_work(self.locals.len() as u64, &WirePath::root())
            .map_err(|_| "origin budget")?;
        if !self.reject_local
            && key == self.key
            && root == self.root
            && path == &self.path
            && origin == &self.origin
            && self.locals.contains(selector)
        {
            Ok(())
        } else {
            Err("wrong local origin")
        }
    }
}
