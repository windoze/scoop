use super::*;

pub(in crate::layout_hir_semantics::tests) struct EmptyFoundation {
    provider: ConeIdentity,
    facts: CanonicalPersistentIdsV1<PersistentExactTypeId>,
    representations: CanonicalPersistentIdsV1<PersistentTypeId>,
    roots: Vec<SourceNominalId>,
    edges: Vec<NominalInheritanceEdgesV1>,
    nominal: Option<NominalFixture>,
}

impl EmptyFoundation {
    pub(in crate::layout_hir_semantics::tests) fn new(provider: ConeIdentity) -> Self {
        Self {
            provider,
            facts: CanonicalPersistentIdsV1::empty(),
            representations: CanonicalPersistentIdsV1::empty(),
            roots: Vec::new(),
            edges: Vec::new(),
            nominal: None,
        }
    }

    pub(in crate::layout_hir_semantics::tests) fn with_missing_root(
        provider: ConeIdentity,
        root: SourceNominalId,
    ) -> Self {
        Self {
            roots: vec![root],
            ..Self::new(provider)
        }
    }

    pub(in crate::layout_hir_semantics::tests) fn with_nominal(nominal: &NominalFixture) -> Self {
        Self {
            provider: nominal.provider,
            facts: CanonicalPersistentIdsV1::try_new(vec![nominal.exact]).unwrap(),
            representations: CanonicalPersistentIdsV1::try_new(vec![nominal.owner]).unwrap(),
            roots: vec![nominal.source],
            edges: vec![nominal.edges.clone()],
            nominal: Some(nominal.clone()),
        }
    }

    pub(in crate::layout_hir_semantics::tests) fn with_dependency(
        provider: ConeIdentity,
        nominal: &NominalFixture,
    ) -> Self {
        Self {
            nominal: Some(nominal.clone()),
            ..Self::new(provider)
        }
    }
}

impl NominalInheritanceSemanticAuthority<TestAuthorityError> for EmptyFoundation {
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
            .ok_or(TestAuthorityError::MissingSourceRoot(owner))
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

impl ExactTypeFactsSemanticAuthority<TestAuthorityError> for EmptyFoundation {
    fn fact_shape(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&ExactTypeFactShapeV1, TestAuthorityError> {
        self.nominal
            .as_ref()
            .filter(|nominal| nominal.exact == exact)
            .map(|nominal| &nominal.fact_shape)
            .ok_or(TestAuthorityError::UnexpectedCall)
    }
}

impl NominalRepresentationSemanticAuthority<TestAuthorityError> for EmptyFoundation {
    fn current_provider(&self) -> ConeIdentity {
        self.provider
    }

    fn required_representation_owners(
        &self,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentTypeId>, TestAuthorityError> {
        Ok(&self.representations)
    }

    fn representation_source(
        &self,
        owner: PersistentTypeId,
    ) -> Result<NominalRepresentationSourceV1<'_>, TestAuthorityError> {
        let nominal = self
            .nominal
            .as_ref()
            .filter(|nominal| nominal.owner == owner)
            .ok_or(TestAuthorityError::UnexpectedCall)?;
        Ok(NominalRepresentationSourceV1 {
            key: &nominal.key,
            access: &nominal.access,
            shape: nominal.representation.shape(),
            public_source_shape: NominalRepresentationPublicSourceShapeV1::NoPublicSourceShape,
        })
    }
}

impl TypeSectionFoundationSemanticAuthority<TestAuthorityError> for EmptyFoundation {
    fn local_exact_facts(
        &self,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentExactTypeId>, TestAuthorityError> {
        Ok(&self.facts)
    }

    fn dependency_facts(&self) -> Result<&[TypeSectionDependencyFactV1], TestAuthorityError> {
        Ok(&[])
    }

    fn local_source_roots(&self) -> Result<&[SourceNominalId], TestAuthorityError> {
        Ok(&self.roots)
    }

    fn local_inheritance_edges(&self) -> Result<&[NominalInheritanceEdgesV1], TestAuthorityError> {
        Ok(&self.edges)
    }

    fn selected_accessor_key(
        &self,
        _accessor: PersistentPropertyAccessorId,
    ) -> Result<&PropertyAccessorKey, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}
