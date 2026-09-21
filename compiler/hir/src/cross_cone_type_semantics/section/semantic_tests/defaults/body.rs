use super::*;

impl ProtectedDefaultOperationTypingSemanticAuthority<&'static str> for DefaultAuthority {
    fn canonical_default_operation_type(
        &mut self,
        _template: &ProtectedDefaultTemplateV1,
        _role: DefaultOperationCoreTypeV1,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<SignatureTypeKey, &'static str> {
        Err("fixture has no default template")
    }
    fn classify_default_core_application(
        &mut self,
        _template: &ProtectedDefaultTemplateV1,
        _value: &SignatureTypeKey,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<Option<DefaultCoreApplicationV1>, &'static str> {
        Err("fixture has no default template")
    }
    fn default_operation_entity_shape(
        &mut self,
        _template: &ProtectedDefaultTemplateV1,
        _entity: DefaultOperationEntityV1<'_>,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<DefaultOperationEntityShapeV1, &'static str> {
        Err("fixture has no default template")
    }
    fn default_operation_type_relation(
        &mut self,
        _template: &ProtectedDefaultTemplateV1,
        _relation: DefaultOperationTypeRelationV1,
        _source: &SignatureTypeKey,
        _target: &SignatureTypeKey,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<bool, &'static str> {
        Err("fixture has no default template")
    }
    fn validate_default_operation_intrinsic(
        &mut self,
        _template: &ProtectedDefaultTemplateV1,
        _intrinsic: DefaultOperationIntrinsicV1<'_>,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<(), &'static str> {
        Err("fixture has no default template")
    }
}

impl ProtectedDefaultLocalDataFlowSemanticAuthority<&'static str> for DefaultAuthority {
    fn default_binding_struct_field_index(
        &mut self,
        _template: &ProtectedDefaultTemplateV1,
        _declaration: PersistentFieldId,
        _owner_type: &SignatureTypeKey,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<u32, &'static str> {
        Err("fixture has no default template")
    }
}

impl ProtectedDefaultNestedCallableSemanticAuthority<&'static str> for DefaultAuthority {
    fn default_nested_callable_identity_shape(
        &mut self,
        _template: &ProtectedDefaultTemplateV1,
        _identity: DefaultNestedCallableIdentityV1,
        _site: crate::DefaultNestedCallableSiteV1,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<DefaultNestedCallableIdentityShapeV1, &'static str> {
        Err("fixture has no default template")
    }
    fn default_nested_callable_abi_shape(
        &mut self,
        _template: &ProtectedDefaultTemplateV1,
        _identity: DefaultNestedCallableIdentityV1,
        _site: crate::DefaultNestedCallableSiteV1,
        _body_arguments: DefaultNestedCallableBodyArgumentsV1<'_>,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<DefaultNestedCallableAbiShapeV1, &'static str> {
        Err("fixture has no default template")
    }
}

impl crate::DefaultLocalFunctionSignatureAuthority<&'static str> for DefaultAuthority {
    fn default_local_function_own_binder_arity(
        &mut self,
        _declaration: scoop_identity::CallableTemplateOrigin,
        _meter: &mut scoop_wire::BudgetMeter,
        _path: &scoop_wire::WirePath,
    ) -> Result<u32, &'static str> {
        Err("fixture has no local function")
    }
}
