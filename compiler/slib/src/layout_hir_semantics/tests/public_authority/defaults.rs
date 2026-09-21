use super::*;

impl DefaultTemplateRootSemanticAuthority<TestAuthorityError> for EmptyPublicAuthority {
    fn default_template_provider_shape(
        &mut self,
        _root: scoop_hir::PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
    ) -> Result<DefaultTemplateProviderShapeV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn default_template_provider_parameter(
        &mut self,
        _root: scoop_hir::PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
    ) -> Result<scoop_hir::DefaultTemplateProviderParameterV1<'_>, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn default_template_provider_receiver(
        &mut self,
        _root: scoop_hir::PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
    ) -> Result<Option<SignatureTypeKey>, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn validate_inherited_default_provider(
        &mut self,
        _key: ExportDefaultTemplateKeyV1,
        _root: scoop_hir::PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
        _mapping: &scoop_hir::CanonicalBinderUseListV1,
    ) -> Result<(), TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl DefaultTemplateOriginSemanticAuthority<TestAuthorityError> for EmptyPublicAuthority {
    fn validate_default_template_origin(
        &mut self,
        _key: ExportDefaultTemplateKeyV1,
        _root: scoop_hir::PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
        _origin: &ExportDefinitionSourceV1,
    ) -> Result<(), TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn validate_default_template_local_origin(
        &mut self,
        _key: ExportDefaultTemplateKeyV1,
        _root: scoop_hir::PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
        _selector: &LocalValueSelector,
        _origin: &ExportDefinitionSourceV1,
    ) -> Result<(), TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl DefaultLocalDataFlowSemanticAuthority<TestAuthorityError> for EmptyPublicAuthority {
    fn default_binding_struct_field_index(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _declaration: PersistentFieldId,
        _owner_type: &SignatureTypeKey,
    ) -> Result<u32, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl DefaultOperationTypingSemanticAuthority<TestAuthorityError> for EmptyPublicAuthority {
    fn canonical_default_operation_type(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _role: DefaultOperationCoreTypeV1,
    ) -> Result<SignatureTypeKey, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn classify_default_core_application(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _value: &SignatureTypeKey,
    ) -> Result<Option<scoop_hir::DefaultCoreApplicationV1>, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn default_operation_entity_shape(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _entity: DefaultOperationEntityV1<'_>,
    ) -> Result<DefaultOperationEntityShapeV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn default_operation_type_relation(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _relation: DefaultOperationTypeRelationV1,
        _source: &SignatureTypeKey,
        _target: &SignatureTypeKey,
    ) -> Result<bool, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn validate_default_operation_intrinsic(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _intrinsic: DefaultOperationIntrinsicV1<'_>,
    ) -> Result<(), TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl DefaultNestedCallableSemanticAuthority<TestAuthorityError> for EmptyPublicAuthority {
    fn default_nested_callable_identity_shape(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _identity: DefaultNestedCallableIdentityV1,
        _site: scoop_hir::DefaultNestedCallableSiteV1,
    ) -> Result<DefaultNestedCallableIdentityShapeV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn default_nested_callable_abi_shape(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _identity: DefaultNestedCallableIdentityV1,
        _site: scoop_hir::DefaultNestedCallableSiteV1,
        _body_arguments: DefaultNestedCallableBodyArgumentsV1<'_>,
    ) -> Result<DefaultNestedCallableAbiShapeV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl DefaultReferenceSemanticAuthority<TestAuthorityError> for EmptyPublicAuthority {
    fn validate_default_callable_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: &ExportDefaultCallableTargetV1,
    ) -> Result<(), TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn validate_default_constructor_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: &DefaultConstructorRefV1,
    ) -> Result<(), TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn validate_default_type_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: &SignatureTypeKey,
    ) -> Result<(), TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn validate_default_global_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: PersistentPropertyId,
    ) -> Result<(), TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn validate_default_singleton_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: PersistentObjectValueId,
    ) -> Result<(), TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn validate_default_field_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: &DefaultFieldRefV1,
    ) -> Result<(), TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}
