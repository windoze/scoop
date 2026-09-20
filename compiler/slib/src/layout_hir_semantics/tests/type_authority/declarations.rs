use super::*;
use std::borrow::Cow;

mod inheritance;

pub(in crate::layout_hir_semantics::tests) struct EmptyDeclarations {
    protected: CanonicalProtectedDeclarationRefsV1,
    inheritance: CanonicalPersistentIdsV1<PersistentExactTypeId>,
    constructors: CanonicalPersistentIdsV1<PersistentConstructorId>,
    nominal: Option<NominalFixture>,
}

impl EmptyDeclarations {
    pub(in crate::layout_hir_semantics::tests) fn new() -> Self {
        Self {
            protected: CanonicalProtectedDeclarationRefsV1::default(),
            inheritance: CanonicalPersistentIdsV1::empty(),
            constructors: CanonicalPersistentIdsV1::empty(),
            nominal: None,
        }
    }

    pub(in crate::layout_hir_semantics::tests) fn with_nominal(nominal: &NominalFixture) -> Self {
        Self {
            protected: CanonicalProtectedDeclarationRefsV1::default(),
            inheritance: CanonicalPersistentIdsV1::try_new(vec![nominal.exact]).unwrap(),
            constructors: CanonicalPersistentIdsV1::empty(),
            nominal: Some(nominal.clone()),
        }
    }
}

impl NominalInheritanceSemanticAuthority<TestAuthorityError> for EmptyDeclarations {
    fn exact_type_key(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&ExactTypeKey, TestAuthorityError> {
        self.nominal
            .as_ref()
            .filter(|nominal| nominal.exact == exact)
            .map(|nominal| &nominal.exact_key)
            .ok_or(TestAuthorityError::UnexpectedCall)
    }

    fn nominal_declaration_key(
        &self,
        owner: SourceNominalId,
    ) -> Result<&SourceDeclarationKey, TestAuthorityError> {
        self.nominal
            .as_ref()
            .filter(|nominal| nominal.source == owner)
            .map(|nominal| &nominal.key)
            .ok_or(TestAuthorityError::UnexpectedCall)
    }

    fn nominal_access_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&DeclarationAccessSourceV1, TestAuthorityError> {
        self.nominal
            .as_ref()
            .filter(|nominal| nominal.source == owner)
            .map(|nominal| &nominal.access)
            .ok_or(TestAuthorityError::UnexpectedCall)
    }

    fn nominal_definition_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, TestAuthorityError> {
        self.nominal
            .as_ref()
            .filter(|nominal| nominal.source == owner)
            .map(|nominal| &nominal.origin)
            .ok_or(TestAuthorityError::UnexpectedCall)
    }

    fn validate_definition_source(
        &self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), TestAuthorityError> {
        self.nominal
            .as_ref()
            .filter(|nominal| &nominal.origin == source)
            .map(|_| ())
            .ok_or(TestAuthorityError::UnexpectedCall)
    }

    fn object_representation(
        &self,
        _owner: PersistentTypeId,
    ) -> Result<&NominalRepresentationSupportV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn generated_nominal_key(
        &self,
        _nominal: PersistentTypeId,
    ) -> Result<&GeneratedNominalKey, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl NominalInterfaceShapeAuthority<TestAuthorityError> for EmptyDeclarations {
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

impl ProtectedCallableSemanticAuthority<TestAuthorityError> for EmptyDeclarations {
    fn callable_source_key(
        &self,
        _declaration: CallableTemplateOrigin,
    ) -> Result<&SourceDeclarationKey, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn property_accessor_key(
        &self,
        _accessor: PersistentPropertyAccessorId,
    ) -> Result<&PropertyAccessorKey, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn property_value_type(
        &self,
        _property: PersistentPropertyId,
    ) -> Result<&SignatureTypeKey, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn unit_type(&self) -> Result<PersistentTypeId, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl ProtectedPropertySemanticAuthority<TestAuthorityError> for EmptyDeclarations {
    fn property_source_key(
        &self,
        _property: PersistentPropertyId,
    ) -> Result<&SourceDeclarationKey, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn property_source_shape(
        &self,
        _property: PersistentPropertyId,
    ) -> Result<ProtectedPropertySourceShapeV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl NominalSupportCallableSemanticAuthority<TestAuthorityError> for EmptyDeclarations {
    fn source_nominal_modality(
        &self,
        owner: SourceNominalId,
    ) -> Result<NominalInheritanceModalityV1, TestAuthorityError> {
        self.nominal
            .as_ref()
            .filter(|nominal| nominal.source == owner)
            .map(|_| NominalInheritanceModalityV1::Final)
            .ok_or(TestAuthorityError::UnexpectedCall)
    }

    fn source_enum_variant_key(
        &self,
        _variant: PersistentEnumVariantId,
    ) -> Result<&EnumVariantIdentityKey, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn source_enum_variant_shape(
        &self,
        _variant: PersistentEnumVariantId,
    ) -> Result<&EnumSourceVariantV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn source_enum_variant_field_key(
        &self,
        _field: PersistentEnumVariantFieldId,
    ) -> Result<&EnumVariantFieldKey, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn source_enum_variant_origin(
        &self,
        _variant: PersistentEnumVariantId,
    ) -> Result<&ExportDefinitionSourceV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl NominalSupportPropertySemanticAuthority<TestAuthorityError> for EmptyDeclarations {
    fn const_source(
        &self,
        _property: PersistentPropertyId,
    ) -> Result<&ConstPropertyDeclarationSourceV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn canonical_const_value_type(
        &self,
        _kind: CanonicalConstValueKindV1,
    ) -> Result<PersistentTypeId, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl NominalSourceShapeSemanticAuthority<TestAuthorityError> for EmptyDeclarations {
    fn struct_field_key(
        &mut self,
        _field: PersistentFieldId,
    ) -> Result<Cow<'_, FieldIdentityKey>, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn enum_variant_key(
        &mut self,
        _variant: PersistentEnumVariantId,
    ) -> Result<Cow<'_, EnumVariantIdentityKey>, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn enum_variant_field_key(
        &mut self,
        _field: PersistentEnumVariantFieldId,
    ) -> Result<Cow<'_, EnumVariantFieldKey>, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn object_value_key(
        &mut self,
        _value: PersistentObjectValueId,
    ) -> Result<Cow<'_, SourceDeclarationKey>, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl ProtectedDeclarationSemanticAuthority<TestAuthorityError> for EmptyDeclarations {
    fn required_protected_declarations(
        &self,
    ) -> Result<&CanonicalProtectedDeclarationRefsV1, TestAuthorityError> {
        Ok(&self.protected)
    }
}
