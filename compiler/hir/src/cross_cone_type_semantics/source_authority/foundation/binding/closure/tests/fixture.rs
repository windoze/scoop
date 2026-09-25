use super::*;

pub(super) struct Artifact {
    coordinate: ConeCoordinate,
    canonical: CanonicalHirFoundation,
    pub source: TypeFoundationSourceAuthorityV1,
    pub owner: PersistentTypeId,
    pub exact: PersistentExactTypeId,
}

pub(super) struct Loaded {
    pub source: TypeFoundationSourceAuthorityV1,
    pub foundation: OdrFreeHirFoundation,
    pub identities: ValidatedIdentityGraph,
    pub owner: PersistentTypeId,
    pub exact: PersistentExactTypeId,
}

impl Artifact {
    pub fn class(name: &str, base: Option<&Loaded>) -> Self {
        Self::new(name, SourceNominalKind::Class, base)
    }

    pub fn new(name: &str, kind: SourceNominalKind, base: Option<&Loaded>) -> Self {
        let coordinate = ConeCoordinate::new("test", name, "1.0.0").unwrap();
        let cone = coordinate.identity().unwrap();
        let key = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                cone,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("Node").unwrap(),
            kind,
            0,
        );
        let nominal = CborIdentityRecord::from_key(key.clone()).unwrap();
        let owner = nominal.id();
        let exact_record = CborIdentityRecord::from_key(ExactTypeKey::Nominal(owner)).unwrap();
        let exact = exact_record.id();
        let file =
            SourceIdentity::new(cone, NormalizedSourcePath::new("main.scoop").unwrap()).unwrap();
        let context = CborIdentityRecord::from_key(SourceContextKey::File {
            source: file.clone(),
        })
        .unwrap();
        let origin =
            DefinitionOrigin::new(file.clone(), SourceSpan::new(0, 4).unwrap(), context.key())
                .unwrap();
        let source_origin = ExportDefinitionSourceV1::new(origin.clone());
        let access = DeclarationAccessSourceV1::try_new(
            DeclaredVisibilityV1::Internal,
            vec![],
            source_origin.clone(),
        )
        .unwrap();
        let mut canonical = CanonicalHirFoundation::empty();
        canonical
            .set_types(vec![
                CoreBuiltinNominal::Unit.identity_record(),
                CoreBuiltinNominal::Any.identity_record(),
                nominal,
            ])
            .unwrap();
        canonical
            .set_sources(vec![SourceRecord::from_utf8(file, "Node", [0, 4]).unwrap()])
            .unwrap();
        canonical.set_source_contexts(vec![context]).unwrap();
        canonical
            .set_definition_origins(vec![DefinitionOriginRecord::new(
                DefinitionOriginSubject::Type(owner),
                origin,
            )])
            .unwrap();
        let mut exacts = vec![exact_record];
        if let Some(base) = base {
            exacts.push(CborIdentityRecord::from_key(ExactTypeKey::Nominal(base.owner)).unwrap());
        }
        let exact_keys =
            CanonicalPersistentIdsV1::try_new(exacts.iter().map(CborIdentityRecord::id).collect())
                .unwrap();
        canonical.set_exact_types(exacts).unwrap();
        let (shape, fact_shape, modality) = match kind {
            SourceNominalKind::Struct => (
                NominalRepresentationShapeV1::Struct {
                    c_layout_policy: NominalCLayoutPolicyV1::Ordinary,
                    fields: vec![],
                },
                ExactTypeFactShapeV1::OrdinaryStruct { fields: vec![] },
                NominalInheritanceModalityV1::Final,
            ),
            SourceNominalKind::Class => (
                NominalRepresentationShapeV1::Class {
                    base: base.map_or(OptionalSignatureType::Absent, |base| {
                        OptionalSignatureType::Present(Box::new(SignatureTypeKey::Nominal(
                            base.owner,
                        )))
                    }),
                    declared_fields: vec![],
                },
                ExactTypeFactShapeV1::Reference,
                NominalInheritanceModalityV1::Open,
            ),
            _ => panic!("fixture requires a class or struct"),
        };
        let source = TypeFoundationSourceAuthorityV1::try_new(TypeFoundationSourceEntriesV1 {
            provider: cone,
            exact_keys,
            sources: CanonicalTypeSourceNominalsV1::try_new(vec![TypeSourceNominalV1::new(
                SourceNominalId::Concrete(owner),
                access.clone(),
            )])
            .unwrap(),
            representations: CanonicalNominalRepresentationSupportV1::try_new(vec![
                NominalRepresentationSupportV1::try_new(&key, access, shape).unwrap(),
            ])
            .unwrap(),
            generated_nominals: CanonicalPersistentIdsV1::empty(),
            accessor_keys: CanonicalPersistentIdsV1::empty(),
            definition_sources: CanonicalExportDefinitionSourcesV1::try_new(vec![source_origin])
                .unwrap(),
            source_roots: CanonicalSourceNominalIdsV1::try_new(vec![SourceNominalId::Concrete(
                owner,
            )])
            .unwrap(),
            local_exact_facts: CanonicalPersistentIdsV1::try_new(vec![exact]).unwrap(),
            dependency_facts: CanonicalTypeSectionDependencyFactsV1::try_new(
                base.into_iter()
                    .map(|base| TypeSectionDependencyFactV1 {
                        provider: base.provider(),
                        exact: base.exact,
                    })
                    .collect(),
            )
            .unwrap(),
            local_inheritance_edges: CanonicalNominalInheritanceEdgesV1::try_new(vec![
                NominalInheritanceEdgesV1::try_new(
                    exact,
                    modality,
                    base.map_or(DirectClassBaseV1::NoClassBase, |base| {
                        DirectClassBaseV1::ClassBase { exact: base.exact }
                    }),
                    vec![],
                )
                .unwrap(),
            ])
            .unwrap(),
            fact_shapes: CanonicalExactTypeFactShapesV1::try_new(vec![
                ExactTypeFactShapeRecordV1::new(exact, fact_shape),
            ])
            .unwrap(),
            representation_owners: CanonicalPersistentIdsV1::try_new(vec![owner]).unwrap(),
        })
        .unwrap();
        Self {
            coordinate,
            canonical,
            source,
            owner,
            exact,
        }
    }

    pub fn load(self, dependency: Option<&Loaded>) -> Loaded {
        let decoded: DecodedHirFoundation =
            decode_canonical(&encode(&self.canonical).unwrap()).unwrap();
        let mut pending = PendingIdentityValidation::new();
        pending.register_authority(ConeIdentity::CORE).unwrap();
        pending
            .register_authority(self.coordinate.identity().unwrap())
            .unwrap();
        if let Some(dependency) = dependency {
            pending.register_authority(dependency.provider()).unwrap();
        }
        decoded.register_identities(&mut pending).unwrap();
        if let Some(dependency) = dependency {
            pending
                .register_external_graph_authorities(&dependency.identities)
                .unwrap();
        }
        decoded.resolve_identities(&mut pending).unwrap();
        let mut identities = pending.finish().unwrap();
        let foundation = OdrFreeHirFoundation::from_validated(
            decoded
                .validate_with_dependency_sources(&self.coordinate, &mut identities)
                .unwrap(),
        )
        .unwrap();
        let source: DecodedTypeFoundationSourceAuthorityV1 =
            decode_canonical(&encode(&self.source).unwrap()).unwrap();
        let source = source.resolve(&mut identities).unwrap();
        Loaded {
            source,
            foundation,
            identities,
            owner: self.owner,
            exact: self.exact,
        }
    }
}

impl Loaded {
    pub fn provider(&self) -> ConeIdentity {
        self.source.entries().provider
    }
    pub fn bind(&self) -> BoundTypeFoundationSourcesV1<'_> {
        self.source
            .bind_to_foundation(&self.foundation, &self.identities)
            .unwrap()
    }
}

pub(super) fn empty_public() -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
        CanonicalCallableSourceInterfacesV1::try_new(vec![]).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(vec![]).unwrap(),
        Default::default(),
        Default::default(),
        Default::default(),
    )
}

pub(super) fn public_proof(
    public: &CrossConeHirInterfaceSectionV1,
    cone: ConeIdentity,
) -> CheckedTypeSectionPublicSupportV1<'_> {
    CheckedTypeSectionPublicSupportV1::validate(
        public,
        cone,
        &CanonicalDirectPublicSurfaceV1::try_new(vec![]).unwrap(),
        &mut crate::cross_cone_interface::EmptyPublicSemanticAuthority(cone),
        &WirePath::root(),
    )
    .unwrap()
}

pub(super) fn provider<'a>(
    bound: &'a BoundTypeFoundationSourcesV1<'a>,
    public: &'a CrossConeHirInterfaceSectionV1,
) -> TypeFoundationSourceProviderV1<'a> {
    TypeFoundationSourceProviderV1::try_new(
        bound,
        public_proof(public, bound.source().entries().provider),
    )
    .unwrap()
}
