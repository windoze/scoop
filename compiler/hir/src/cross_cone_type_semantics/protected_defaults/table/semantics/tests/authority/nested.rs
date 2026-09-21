use super::*;

impl ProtectedDefaultNestedCallableSemanticAuthority<&'static str> for Authority<'_> {
    fn default_nested_callable_identity_shape(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        identity: DefaultNestedCallableIdentityV1,
        _site: crate::DefaultNestedCallableSiteV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultNestedCallableIdentityShapeV1, &'static str> {
        self.check(template, meter, path)?;
        self.nested_calls += 1;
        if !self.case.nested()
            || identity != DefaultNestedCallableIdentityV1::Lambda(self.fixture.lambda)
        {
            return Err("unknown generated nested identity");
        }
        Ok(DefaultNestedCallableIdentityShapeV1::new(
            DefaultNestedCallableProvenanceV1::TemplateLexical,
            nested_path(StructuralDefinitionSiteRole::Lambda),
            0,
            DefaultNestedCallableBodyShapeV1::Lexical,
        ))
    }
    fn default_nested_callable_abi_shape(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        identity: DefaultNestedCallableIdentityV1,
        _site: crate::DefaultNestedCallableSiteV1,
        arguments: DefaultNestedCallableBodyArgumentsV1<'_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultNestedCallableAbiShapeV1, &'static str> {
        self.check(template, meter, path)?;
        self.nested_calls += 1;
        if identity != DefaultNestedCallableIdentityV1::Lambda(self.fixture.lambda)
            || !matches!(arguments, DefaultNestedCallableBodyArgumentsV1::Lexical)
        {
            return Err("wrong nested body binding");
        }
        Ok(DefaultNestedCallableAbiShapeV1::new(
            self.fixture.function_type(),
            if self.case == Case::NestedAbi {
                vec![]
            } else {
                vec![self.fixture.unit.clone()]
            },
        ))
    }
}
