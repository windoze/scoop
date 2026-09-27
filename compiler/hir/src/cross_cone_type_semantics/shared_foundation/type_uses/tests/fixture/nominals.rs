use super::*;

impl Artifact {
    pub fn nominal(
        &mut self,
        name: &str,
        kind: SourceNominalKind,
        parents: &[PersistentTypeId],
    ) -> PersistentTypeId {
        let provider = self.coordinate.identity().unwrap();
        let key = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                provider,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new(name).unwrap(),
            kind,
            0,
        );
        let identity = CborIdentityRecord::from_key(key).unwrap();
        let owner = identity.id();
        let (kind, shape, modality) = match kind {
            SourceNominalKind::Class => (
                PublicNominalKindV1::Class,
                NominalSourceShapeV1::Class(Default::default()),
                NominalInheritanceModalityV1::Open,
            ),
            SourceNominalKind::Interface => (
                PublicNominalKindV1::Interface,
                NominalSourceShapeV1::Interface,
                NominalInheritanceModalityV1::Interface,
            ),
            SourceNominalKind::Struct => (
                PublicNominalKindV1::Struct,
                NominalSourceShapeV1::Struct(
                    StructSourceShapeV1::try_new(
                        Vec::new(),
                        NominalCLayoutPolicyV1::Ordinary,
                        false,
                    )
                    .unwrap(),
                ),
                NominalInheritanceModalityV1::Final,
            ),
            SourceNominalKind::Enum => (
                PublicNominalKindV1::Enum,
                NominalSourceShapeV1::Enum(EnumSourceShapeV1::try_new(Vec::new()).unwrap()),
                NominalInheritanceModalityV1::Final,
            ),
            SourceNominalKind::Object => (
                PublicNominalKindV1::Object,
                NominalSourceShapeV1::Object(ObjectSourceShapeV1::new(
                    crate::ObjectSourceKindV1::Standalone,
                    scoop_identity::PersistentObjectValueId::from_source_object(identity.key())
                        .unwrap(),
                    Default::default(),
                )),
                NominalInheritanceModalityV1::Final,
            ),
            SourceNominalKind::AnnotationClass => {
                panic!("call fixtures require runtime nominal declarations")
            }
        };
        let parents = parents
            .iter()
            .map(|parent| SignatureTypeKey::Nominal(*parent))
            .collect::<Vec<_>>();
        let dispatch = if kind == PublicNominalKindV1::Interface {
            NominalDispatchOrderV1::Interface {
                parents: parents.clone(),
                members: Vec::new(),
            }
        } else {
            NominalDispatchOrderV1::empty(kind)
        };
        let details = NominalDeclarationDetailsV1::new(
            modality,
            DeclaredVisibilityV1::Public,
            CanonicalPersistentIdsV1::empty(),
            CanonicalNestedMemberRefsV1::try_new(Vec::new()).unwrap(),
            Default::default(),
            dispatch,
            CanonicalNominalDispatchSelectionsV1::empty(),
            None,
        );
        let nominal = NominalInterfaceRecordV1::try_new(
            SourceNominalId::Concrete(owner),
            kind,
            CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
            CanonicalSignatureTypesV1::try_new(parents).unwrap(),
            CanonicalPersistentIdsV1::empty(),
            Default::default(),
            CanonicalPersistentIdsV1::empty(),
            shape,
            details,
        )
        .unwrap();
        self.nominals.push((identity, nominal));
        owner
    }
}
