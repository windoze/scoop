use super::*;

impl ProtectedDefaultLocalDataFlowSemanticAuthority<&'static str> for Authority<'_> {
    fn default_binding_struct_field_index(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        _declaration: PersistentFieldId,
        _owner: &SignatureTypeKey,
    ) -> Result<u32, &'static str> {
        assert!(std::ptr::eq(self.template, template));
        Err("fixture has no projected struct binding")
    }
}
impl ProtectedDefaultOperationTypingSemanticAuthority<&'static str> for Authority<'_> {
    fn canonical_default_operation_type(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        role: DefaultOperationCoreTypeV1,

        path: &WirePath,
    ) -> Result<SignatureTypeKey, &'static str> {
        self.check(template, path)?;
        self.body_calls += 1;
        if role == DefaultOperationCoreTypeV1::Unit {
            Ok(self.fixture.unit.clone())
        } else {
            Err("unknown fixture core type")
        }
    }
    fn classify_default_core_application(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        _value: &SignatureTypeKey,

        path: &WirePath,
    ) -> Result<Option<DefaultCoreApplicationV1>, &'static str> {
        self.check(template, path)?;
        self.body_calls += 1;
        Ok(None)
    }
    fn default_operation_entity_shape(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        _entity: DefaultOperationEntityV1<'_>,

        path: &WirePath,
    ) -> Result<DefaultOperationEntityShapeV1, &'static str> {
        self.check(template, path)?;
        Err("fixture has no operation entity")
    }
    fn default_operation_type_relation(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        _relation: DefaultOperationTypeRelationV1,
        source: &SignatureTypeKey,
        target: &SignatureTypeKey,

        path: &WirePath,
    ) -> Result<bool, &'static str> {
        self.check(template, path)?;
        self.body_calls += 1;
        Ok(source == target)
    }
    fn validate_default_operation_intrinsic(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        _intrinsic: DefaultOperationIntrinsicV1<'_>,

        path: &WirePath,
    ) -> Result<(), &'static str> {
        self.check(template, path)?;
        Err("fixture has no compiler intrinsic")
    }
}
