use super::*;

#[derive(Clone)]
pub(in crate::layout_hir_semantics::tests) struct NominalFixture {
    pub(super) provider: ConeIdentity,
    pub(super) source_identity: SourceIdentity,
    pub(super) context: SourceContextKey,
    pub(super) source: SourceNominalId,
    pub(super) owner: PersistentTypeId,
    pub(super) key: SourceDeclarationKey,
    pub(in crate::layout_hir_semantics::tests) exact: PersistentExactTypeId,
    pub(super) exact_key: ExactTypeKey,
    pub(super) origin: ExportDefinitionSourceV1,
    pub(super) access: DeclarationAccessSourceV1,
    pub(super) edges: NominalInheritanceEdgesV1,
    pub(super) representation: NominalRepresentationSupportV1,
    pub(super) fact_shape: ExactTypeFactShapeV1,
    pub(super) schemas: CanonicalInheritanceSlotSchemasV1,
}

impl NominalFixture {
    pub(in crate::layout_hir_semantics::tests) fn new(provider: ConeIdentity) -> Self {
        let source_identity = SourceIdentity::new(
            provider,
            NormalizedSourcePath::new("semantic.scoop").unwrap(),
        )
        .unwrap();
        let context = SourceContextKey::File {
            source: source_identity.clone(),
        };
        let origin = ExportDefinitionSourceV1::new(
            scoop_identity::DefinitionOrigin::new(
                source_identity.clone(),
                SourceSpan::new(0, 5).unwrap(),
                &context,
            )
            .unwrap(),
        );
        let key = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                provider,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("LocalNode").unwrap(),
            SourceNominalKind::Class,
            0,
        );
        let owner = PersistentTypeId::from_source_declaration(&key).unwrap();
        let source = SourceNominalId::Concrete(owner);
        let exact_key = ExactTypeKey::Nominal(owner);
        let exact = PersistentExactTypeId::from_key(&exact_key).unwrap();
        let access = DeclarationAccessSourceV1::try_new(
            DeclaredVisibilityV1::Internal,
            Vec::new(),
            origin.clone(),
        )
        .unwrap();
        let edges = NominalInheritanceEdgesV1::try_new(
            exact,
            NominalInheritanceModalityV1::Final,
            DirectClassBaseV1::NoClassBase,
            Vec::new(),
        )
        .unwrap();
        let representation = NominalRepresentationSupportV1::try_new(
            &key,
            access.clone(),
            NominalRepresentationShapeV1::Class {
                base: OptionalSignatureType::Absent,
                declared_fields: Vec::new(),
            },
        )
        .unwrap();
        let schemas = CanonicalInheritanceSlotSchemasV1::try_new(vec![
            InheritanceSlotSchemaV1::try_new(InheritanceSlotSchemaRoleV1::ClassVtable, Vec::new())
                .unwrap(),
        ])
        .unwrap();
        Self {
            provider,
            source_identity,
            context,
            source,
            owner,
            key,
            exact,
            exact_key,
            origin,
            access,
            edges,
            representation,
            fact_shape: ExactTypeFactShapeV1::Reference,
            schemas,
        }
    }

    pub(in crate::layout_hir_semantics::tests) const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub(in crate::layout_hir_semantics::tests) fn foundation(&self) -> CanonicalHirFoundation {
        let mut foundation = CanonicalHirFoundation::empty();
        foundation
            .set_sources(vec![
                SourceRecord::from_utf8(self.source_identity.clone(), "class LocalNode", [0, 5])
                    .unwrap(),
            ])
            .unwrap();
        foundation
            .set_source_contexts(vec![
                CborIdentityRecord::from_key(self.context.clone()).unwrap(),
            ])
            .unwrap();
        foundation
            .set_definition_origins(vec![DefinitionOriginRecord::new(
                DefinitionOriginSubject::Type(self.owner()),
                self.origin.origin().clone(),
            )])
            .unwrap();
        foundation
            .set_types(vec![
                CoreBuiltinNominal::Unit.identity_record(),
                CoreBuiltinNominal::Any.identity_record(),
                CborIdentityRecord::from_key(self.key.clone()).unwrap(),
            ])
            .unwrap();
        foundation
            .set_exact_types(vec![
                CborIdentityRecord::from_key(self.exact_key.clone()).unwrap(),
            ])
            .unwrap();
        foundation
    }

    pub(in crate::layout_hir_semantics::tests) fn section(
        &self,
    ) -> CrossConeTypeSemanticsSectionV1 {
        let mut meter = BudgetMeter::new(scoop_wire::DecodeLimits::default());
        let graph = CheckedNominalInheritanceGraphV1::validate_with_source_roots(
            [&self.edges],
            [self.source],
            self,
            &mut meter,
        )
        .unwrap();
        let inheritance = NominalInheritanceInterfaceV1::try_new(
            self.edges.clone(),
            graph
                .replay_nominal_domains(self.exact, &mut meter)
                .unwrap()
                .to_record(),
            CanonicalInheritanceConstructorsV1::try_new(Vec::new()).unwrap(),
            CanonicalInheritanceSlotContractsV1::try_new(Vec::new()).unwrap(),
            CanonicalProtectedDeclarationRefsV1::default(),
            self.schemas.clone(),
        )
        .unwrap();
        CrossConeTypeSemanticsSectionV1::new(
            CanonicalExactTypeFactsV1::try_new(vec![
                ExactTypeFactsV1::try_new(
                    self.exact,
                    ExactTypeKindV1::Reference,
                    ExactTypeGcV1::ContainsManagedReferences,
                )
                .unwrap(),
            ])
            .unwrap(),
            CanonicalNominalRepresentationSupportV1::try_new(vec![self.representation.clone()])
                .unwrap(),
            CanonicalNominalInheritanceInterfacesV1::try_new(vec![inheritance]).unwrap(),
            CanonicalProtectedDeclarationInterfacesV1::try_new(Vec::new()).unwrap(),
            CanonicalProtectedCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
            CanonicalProtectedDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
            CanonicalExportDefinitionSourcesV1::try_new(vec![self.origin.clone()]).unwrap(),
            CanonicalSelectedExternalTypeUsesV1::try_new(Vec::new()).unwrap(),
        )
    }

    pub(in crate::layout_hir_semantics::tests) const fn owner(&self) -> PersistentTypeId {
        self.owner
    }
}

impl NominalInheritanceSemanticAuthority<TestAuthorityError> for NominalFixture {
    fn exact_type_key(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&ExactTypeKey, TestAuthorityError> {
        (exact == self.exact)
            .then_some(&self.exact_key)
            .ok_or(TestAuthorityError::UnexpectedCall)
    }

    fn nominal_declaration_key(
        &self,
        owner: SourceNominalId,
    ) -> Result<&SourceDeclarationKey, TestAuthorityError> {
        (owner == self.source)
            .then_some(&self.key)
            .ok_or(TestAuthorityError::MissingSourceRoot(owner))
    }

    fn nominal_access_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&DeclarationAccessSourceV1, TestAuthorityError> {
        (owner == self.source)
            .then_some(&self.access)
            .ok_or(TestAuthorityError::UnexpectedCall)
    }

    fn nominal_definition_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, TestAuthorityError> {
        (owner == self.source)
            .then_some(&self.origin)
            .ok_or(TestAuthorityError::UnexpectedCall)
    }

    fn validate_definition_source(
        &self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), TestAuthorityError> {
        (source == &self.origin)
            .then_some(())
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
