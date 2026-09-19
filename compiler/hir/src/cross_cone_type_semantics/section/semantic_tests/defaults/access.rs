use super::*;

impl ProtectedDefaultReferenceAccessSemanticAuthority<&'static str> for DefaultAuthority {
    fn replay_param_free_default_reference<'g, 'a>(
        &mut self,
        _source_use: ProtectedDefaultReferenceSourceUseV1<'_, '_, '_>,
        _graph: &'g CheckedNominalInheritanceGraphV1<'a>,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<CheckedPersistentAccessDomainV1<'g, 'a>, &'static str> {
        Err("fixture has no default template")
    }
    fn validate_generic_default_reference(
        &mut self,
        _source_use: ProtectedDefaultReferenceSourceUseV1<'_, '_, '_>,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<(), &'static str> {
        Err("fixture has no default template")
    }
}

impl ProtectedDefaultSourceProfileSemanticAuthority<&'static str> for DefaultAuthority {
    fn default_access_profile(
        &self,
        _key: ProtectedDefaultTemplateKeyV1,
    ) -> Result<ProtectedDefaultWitnessSourceProfileV1, &'static str> {
        Err("fixture has no default template")
    }
}

impl ProtectedDefaultRootSlotSemanticAuthority<&'static str> for DefaultAuthority {
    fn default_root_slots(
        &self,
        _owner: CallableTemplateOrigin,
    ) -> Result<&CanonicalProtectedSlotRefsV1, &'static str> {
        Err("fixture has no default template")
    }
}
