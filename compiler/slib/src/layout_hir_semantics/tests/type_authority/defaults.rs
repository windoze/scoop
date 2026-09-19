use super::*;

mod body;

pub(in crate::layout_hir_semantics::tests) struct EmptyDefaults {
    provider: ConeIdentity,
    origins: Vec<ExportDefinitionSourceV1>,
}

impl EmptyDefaults {
    pub(in crate::layout_hir_semantics::tests) fn new(provider: ConeIdentity) -> Self {
        Self {
            provider,
            origins: Vec::new(),
        }
    }

    pub(in crate::layout_hir_semantics::tests) fn with_nominal(nominal: &NominalFixture) -> Self {
        Self {
            provider: nominal.provider,
            origins: vec![nominal.origin.clone()],
        }
    }
}

impl ExportDefinitionSourceSemanticAuthority<TestAuthorityError> for EmptyDefaults {
    fn current_cone(&self) -> ConeIdentity {
        self.provider
    }

    fn validate_export_definition_source(
        &mut self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), TestAuthorityError> {
        self.origins
            .contains(source)
            .then_some(())
            .ok_or(TestAuthorityError::UnexpectedCall)
    }
}

impl TypeDefinitionSourceSemanticAuthority<TestAuthorityError> for EmptyDefaults {
    fn validate_type_definition_source_use(
        &mut self,
        _source_use: TypeDefinitionSourceUseV1<'_>,
        source: &ExportDefinitionSourceV1,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<(), TestAuthorityError> {
        self.validate_export_definition_source(source)
    }
}

impl NominalInterfaceShapeAuthority<TestAuthorityError> for EmptyDefaults {
    fn concrete_nominal_shape(
        &mut self,
        _declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn generic_nominal_shape(
        &mut self,
        _declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl ProtectedSourceProtocolSemanticAuthority<TestAuthorityError> for EmptyDefaults {
    fn canonical_array_type(&mut self) -> Result<PersistentGenericTypeId, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn source_parameter_shape(
        &self,
        _owner: CallableTemplateOrigin,
        _position: u32,
    ) -> Result<&SourceParameterShapeV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn source_parameter_calling_kind(
        &self,
        _owner: CallableTemplateOrigin,
        _position: u32,
    ) -> Result<ProtectedParameterCallingKindV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn validate_source_parameter_origin(
        &self,
        _owner: CallableTemplateOrigin,
        _position: u32,
        _origin: &ExportDefinitionSourceV1,
    ) -> Result<(), TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl ProtectedDefaultRootSemanticAuthority<TestAuthorityError> for EmptyDefaults {
    fn protected_default_provider_shape(
        &mut self,
        _root: PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
        _meter: &mut BudgetMeter,
    ) -> Result<DefaultTemplateProviderShapeV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn validate_inherited_protected_default_provider(
        &mut self,
        _key: ProtectedDefaultTemplateKeyV1,
        _root: PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
        _meter: &mut BudgetMeter,
    ) -> Result<(), TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl ProtectedDefaultOriginSemanticAuthority<TestAuthorityError> for EmptyDefaults {
    fn validate_protected_default_origin(
        &mut self,
        _key: ProtectedDefaultTemplateKeyV1,
        _root: PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
        _origin: &ExportDefinitionSourceV1,
        _meter: &mut BudgetMeter,
    ) -> Result<(), TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn validate_protected_default_local_origin(
        &mut self,
        _key: ProtectedDefaultTemplateKeyV1,
        _root: PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
        _selector: &LocalValueSelector,
        _origin: &ExportDefinitionSourceV1,
        _meter: &mut BudgetMeter,
    ) -> Result<(), TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl ProtectedDefaultSourceProfileSemanticAuthority<TestAuthorityError> for EmptyDefaults {
    fn default_access_profile(
        &self,
        _key: ProtectedDefaultTemplateKeyV1,
    ) -> Result<ProtectedDefaultWitnessSourceProfileV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl ProtectedDefaultRootSlotSemanticAuthority<TestAuthorityError> for EmptyDefaults {
    fn default_root_slots(
        &self,
        _owner: CallableTemplateOrigin,
    ) -> Result<&CanonicalProtectedSlotRefsV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl ProtectedDefaultReferenceAccessSemanticAuthority<TestAuthorityError> for EmptyDefaults {
    fn replay_param_free_default_reference<'graph, 'section>(
        &mut self,
        _source_use: ProtectedDefaultReferenceSourceUseV1<'_, '_, '_>,
        _graph: &'graph CheckedNominalInheritanceGraphV1<'section>,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<CheckedPersistentAccessDomainV1<'graph, 'section>, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn validate_generic_default_reference(
        &mut self,
        _source_use: ProtectedDefaultReferenceSourceUseV1<'_, '_, '_>,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<(), TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}
