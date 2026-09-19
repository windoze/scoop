use super::*;

impl ProtectedDefaultOperationTypingSemanticAuthority<TestAuthorityError> for EmptyDefaults {
    fn canonical_default_operation_type(
        &mut self,
        _template: &ProtectedDefaultTemplateV1,
        _role: DefaultOperationCoreTypeV1,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<SignatureTypeKey, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn classify_default_core_application(
        &mut self,
        _template: &ProtectedDefaultTemplateV1,
        _value: &SignatureTypeKey,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<Option<DefaultCoreApplicationV1>, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn default_operation_entity_shape(
        &mut self,
        _template: &ProtectedDefaultTemplateV1,
        _entity: DefaultOperationEntityV1<'_>,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<DefaultOperationEntityShapeV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn default_operation_type_relation(
        &mut self,
        _template: &ProtectedDefaultTemplateV1,
        _relation: DefaultOperationTypeRelationV1,
        _source: &SignatureTypeKey,
        _target: &SignatureTypeKey,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<bool, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn validate_default_operation_intrinsic(
        &mut self,
        _template: &ProtectedDefaultTemplateV1,
        _intrinsic: DefaultOperationIntrinsicV1<'_>,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<(), TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl ProtectedDefaultLocalDataFlowSemanticAuthority<TestAuthorityError> for EmptyDefaults {
    fn default_binding_struct_field_index(
        &mut self,
        _template: &ProtectedDefaultTemplateV1,
        _declaration: PersistentFieldId,
        _owner_type: &SignatureTypeKey,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<u32, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl ProtectedDefaultNestedCallableSemanticAuthority<TestAuthorityError> for EmptyDefaults {
    fn default_nested_callable_identity_shape(
        &mut self,
        _template: &ProtectedDefaultTemplateV1,
        _identity: DefaultNestedCallableIdentityV1,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<DefaultNestedCallableIdentityShapeV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn default_nested_callable_abi_shape(
        &mut self,
        _template: &ProtectedDefaultTemplateV1,
        _identity: DefaultNestedCallableIdentityV1,
        _body_arguments: DefaultNestedCallableBodyArgumentsV1<'_>,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<DefaultNestedCallableAbiShapeV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}
