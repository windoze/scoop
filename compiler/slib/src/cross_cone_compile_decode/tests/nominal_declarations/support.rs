use super::*;

pub(super) struct Fixture {
    foundation: CanonicalHirFoundation,
    pub public: NominalInterfaceRecordV1,
    pub hidden: NominalInterfaceRecordV1,
    extra: NominalInterfaceRecordV1,
}

impl Fixture {
    pub fn identities(&self) -> scoop_identity::ValidatedIdentityGraph {
        let decoded: scoop_hir::DecodedHirFoundation = scoop_wire::decode_canonical(
            &encode(&self.foundation).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        let mut pending = scoop_identity::PendingIdentityValidation::new();
        pending.register_authority(cone().identity()).unwrap();
        pending
            .register_authority(scoop_identity::ConeIdentity::CORE)
            .unwrap();
        decoded.register_identities(&mut pending).unwrap();
        decoded.resolve_identities(&mut pending).unwrap();
        pending.finish().unwrap()
    }

    pub fn new() -> Self {
        let cone = cone().identity();
        let public = nominal_key(cone, "Container", DefinitionOwnerChain::top_level());
        let hidden = nominal_key(
            cone,
            "Hidden",
            DefinitionOwnerChain::from_outer_to_inner(vec![DefinitionOwnerAtom::Type(public.id())]),
        );
        let extra = nominal_key(cone, "Unrelated", DefinitionOwnerChain::top_level());
        let source = SourceIdentity::new(
            cone,
            NormalizedSourcePath::new("src/Container.scoop").unwrap(),
        )
        .unwrap();
        let context_key = SourceContextKey::File {
            source: source.clone(),
        };
        let context =
            CborIdentityRecord::<PersistentSourceContextId, _>::from_key(context_key.clone())
                .unwrap();
        let origin =
            DefinitionOrigin::new(source.clone(), SourceSpan::new(0, 5).unwrap(), &context_key)
                .unwrap();
        let mut foundation = base_hir_foundation();
        foundation
            .set_sources(vec![
                scoop_hir::SourceRecord::from_utf8(source, "class Container", [0, 5]).unwrap(),
            ])
            .unwrap();
        foundation.set_source_contexts(vec![context]).unwrap();
        foundation
            .set_definition_origins(
                [&public, &hidden, &extra]
                    .map(|key| {
                        DefinitionOriginRecord::new(
                            DefinitionOriginSubject::Type(key.id()),
                            origin.clone(),
                        )
                    })
                    .to_vec(),
            )
            .unwrap();
        foundation
            .set_types(vec![
                CoreBuiltinNominal::Unit.identity_record(),
                CoreBuiltinNominal::Any.identity_record(),
                public.clone(),
                hidden.clone(),
                extra.clone(),
            ])
            .unwrap();
        Self {
            foundation,
            public: record(
                public.id(),
                DeclaredVisibilityV1::Public,
                vec![SourceNominalId::Concrete(hidden.id())],
            ),
            hidden: record(hidden.id(), DeclaredVisibilityV1::Private, vec![]),
            extra: record(extra.id(), DeclaredVisibilityV1::Private, vec![]),
        }
    }

    pub fn artifact(&self, hidden: bool, child: bool, extra: bool) -> Vec<u8> {
        let public = if child {
            self.public.clone()
        } else {
            let SourceNominalId::Concrete(id) = self.public.declaration() else {
                panic!("concrete fixture")
            };
            record(id, DeclaredVisibilityV1::Public, vec![])
        };
        let mut support = Vec::new();
        if hidden {
            support.push(self.hidden.clone());
        }
        if extra {
            support.push(self.extra.clone());
        }
        let mut section = CrossConeHirInterfaceSectionV1::new(
            CanonicalPublicExportBindingsV1::try_new(vec![]).unwrap(),
            CanonicalNominalInterfacesV1::with_support(vec![public], support).unwrap(),
            CanonicalCallableInterfacesV1::try_new(vec![]).unwrap(),
            CanonicalPropertyInterfacesV1::try_new(vec![]).unwrap(),
            CanonicalTypeAliasInterfacesV1::try_new(vec![]).unwrap(),
            CanonicalCallableSourceInterfacesV1::try_new(vec![]).unwrap(),
            CanonicalExportDefaultTemplatesV1::try_new(vec![]).unwrap(),
            CanonicalExportConstValuesV1::try_new(vec![]).unwrap(),
            CanonicalExportDefinitionSourcesV1::try_new(vec![]).unwrap(),
            CanonicalExternalHirReferencesV1::try_new(vec![]).unwrap(),
        );
        cross_cone_artifact_for_with_hir_foundation(
            cone(),
            vec![],
            &self.foundation,
            encode(&section.index_for_wire().unwrap()).unwrap(),
        )
    }
}

fn nominal_key(
    cone: scoop_identity::ConeIdentity,
    name: &str,
    owners: DefinitionOwnerChain,
) -> CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            cone,
            PackagePath::root(),
            owners,
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    ))
    .unwrap()
}

fn record(
    id: PersistentTypeId,
    visibility: DeclaredVisibilityV1,
    children: Vec<SourceNominalId>,
) -> NominalInterfaceRecordV1 {
    NominalInterfaceRecordV1::try_new(
        SourceNominalId::Concrete(id),
        PublicNominalKindV1::Class,
        CanonicalBinderListV1::try_new(vec![]).unwrap(),
        CanonicalSignatureTypesV1::try_new(vec![]).unwrap(),
        CanonicalPersistentIdsV1::empty(),
        CanonicalPublicMemberRefsV1::default(),
        CanonicalPersistentIdsV1::empty(),
        NominalSourceShapeV1::Class(Default::default()),
        NominalDeclarationDetailsV1::new(
            NominalInheritanceModalityV1::Final,
            visibility,
            CanonicalPersistentIdsV1::empty(),
            CanonicalNestedMemberRefsV1::try_new(vec![]).unwrap(),
            CanonicalNestedNominalRefsV1::try_new(children).unwrap(),
        ),
    )
    .unwrap()
}
