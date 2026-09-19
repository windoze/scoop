use super::*;

mod body;
mod nested;
mod origins;
mod references;

pub(super) struct Authority<'a> {
    pub fixture: &'a Fixture,
    pub source: SourceFixture,
    pub template: &'a ProtectedDefaultTemplateV1,
    pub resources: *const BudgetMeter,
    pub path: &'a WirePath,
    pub case: Case,
    pub provider_calls: usize,
    pub origin_calls: usize,
    pub body_calls: usize,
    pub metadata_calls: usize,
    pub expression_calls: usize,
    pub concrete_calls: usize,
    pub nested_calls: usize,
    pub slots: CanonicalProtectedSlotRefsV1,
}
impl<'a> Authority<'a> {
    pub fn new(
        fixture: &'a Fixture,
        template: &'a ProtectedDefaultTemplateV1,
        meter: &BudgetMeter,
        path: &'a WirePath,
        case: Case,
    ) -> Self {
        Self {
            fixture,
            source: fixture.source.clone(),
            template,
            resources: std::ptr::from_ref(meter),
            path,
            case,
            provider_calls: 0,
            origin_calls: 0,
            body_calls: 0,
            metadata_calls: 0,
            expression_calls: 0,
            concrete_calls: 0,
            nested_calls: 0,
            slots: CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
        }
    }
    fn check(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), &'static str> {
        assert!(std::ptr::eq(self.template, template));
        assert!(std::ptr::eq(self.resources, meter));
        assert!(std::ptr::eq(self.path, path));
        meter
            .charge_work(7, path)
            .map_err(|_| "shared authority budget")
    }
    fn check_source(
        &self,
        key: ProtectedDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        meter: &mut BudgetMeter,
    ) -> Result<(), &'static str> {
        assert!(std::ptr::eq(self.resources, meter));
        meter
            .charge_work(3, self.path)
            .map_err(|_| "source authority budget")?;
        if key == self.fixture.key
            && root.declaration() == key.owner()
            && path == &definition_path()
        {
            Ok(())
        } else {
            Err("wrong source default identity")
        }
    }
}
impl NominalInterfaceShapeAuthority<&'static str> for Authority<'_> {
    fn concrete_nominal_shape(
        &mut self,
        id: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, &'static str> {
        self.source.concrete_nominal_shape(id)
    }
    fn generic_nominal_shape(
        &mut self,
        id: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, &'static str> {
        self.source.generic_nominal_shape(id)
    }
}
impl ProtectedDefaultRootSemanticAuthority<&'static str> for Authority<'_> {
    fn protected_default_provider_shape(
        &mut self,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        meter: &mut BudgetMeter,
    ) -> Result<DefaultTemplateProviderShapeV1, &'static str> {
        self.check_source(self.fixture.key, root, path, meter)?;
        self.provider_calls += 1;
        DefaultTemplateProviderShapeV1::try_new(u32::from(self.case.generic()), 0)
            .map_err(|_| "invalid provider binders")
    }
    fn validate_inherited_protected_default_provider(
        &mut self,
        _key: ProtectedDefaultTemplateKeyV1,
        _root: PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
        _meter: &mut BudgetMeter,
    ) -> Result<(), &'static str> {
        Err("fixture has no inherited default provider")
    }
}
impl ProtectedDefaultSourceProfileSemanticAuthority<&'static str> for Authority<'_> {
    fn default_access_profile(
        &self,
        key: ProtectedDefaultTemplateKeyV1,
    ) -> Result<ProtectedDefaultWitnessSourceProfileV1, &'static str> {
        if key != self.fixture.key {
            return Err("unknown source default profile");
        }
        Ok(if self.case.generic() {
            ProtectedDefaultWitnessSourceProfileV1::GenericSourceMetadata
        } else {
            ProtectedDefaultWitnessSourceProfileV1::ParamFree
        })
    }
}
impl ProtectedDefaultRootSlotSemanticAuthority<&'static str> for Authority<'_> {
    fn default_root_slots(
        &self,
        owner: CallableTemplateOrigin,
    ) -> Result<&CanonicalProtectedSlotRefsV1, &'static str> {
        if owner == self.fixture.key.owner() {
            Ok(&self.slots)
        } else {
            Err("unknown default slot owner")
        }
    }
}
