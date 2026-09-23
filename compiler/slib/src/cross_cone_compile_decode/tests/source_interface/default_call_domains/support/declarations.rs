use super::*;

impl Kind {
    pub(super) fn source(self) -> SourceNominalKind {
        match self {
            Self::Class => SourceNominalKind::Class,
            Self::Interface => SourceNominalKind::Interface,
            Self::Struct => SourceNominalKind::Struct,
        }
    }
    fn public(self) -> PublicNominalKindV1 {
        match self {
            Self::Class => PublicNominalKindV1::Class,
            Self::Interface => PublicNominalKindV1::Interface,
            Self::Struct => PublicNominalKindV1::Struct,
        }
    }
    fn shape(self) -> NominalSourceShapeV1 {
        match self {
            Self::Class => NominalSourceShapeV1::Class(Default::default()),
            Self::Interface => NominalSourceShapeV1::Interface,
            Self::Struct => NominalSourceShapeV1::Struct(
                StructSourceShapeV1::try_new(vec![], NominalCLayoutPolicyV1::Ordinary).unwrap(),
            ),
        }
    }
    fn modality(self) -> NominalInheritanceModalityV1 {
        match self {
            Self::Class => NominalInheritanceModalityV1::Open,
            Self::Interface => NominalInheritanceModalityV1::Interface,
            Self::Struct => NominalInheritanceModalityV1::Final,
        }
    }
}

impl Nominal {
    pub(super) fn record(&self, all: &[Nominal]) -> NominalInterfaceRecordV1 {
        let children = all
            .iter()
            .filter(|record| record.key.owners().owners().last() == Some(&owner_atom(self.id)))
            .map(|record| record.id)
            .collect();
        NominalInterfaceRecordV1::try_new(
            self.id,
            self.kind.public(),
            CanonicalBinderListV1::try_new(
                (0..self.arity)
                    .map(|index| {
                        TypeParameterBinderV1::new(
                            CanonicalIdentifier::new(&format!("T{index}")).unwrap(),
                            TypeParameterBoundsV1::Unconstrained,
                        )
                    })
                    .collect(),
            )
            .unwrap(),
            CanonicalSignatureTypesV1::try_new(self.parents.clone()).unwrap(),
            CanonicalPersistentIdsV1::empty(),
            CanonicalPublicMemberRefsV1::default(),
            CanonicalPersistentIdsV1::empty(),
            self.kind.shape(),
            NominalDeclarationDetailsV1::new(
                self.kind.modality(),
                self.visibility,
                CanonicalPersistentIdsV1::empty(),
                CanonicalNestedMemberRefsV1::try_new(self.members.clone()).unwrap(),
                CanonicalNestedNominalRefsV1::try_new(children).unwrap(),
            ),
        )
        .unwrap()
    }
}

pub(in super::super) fn applied(
    id: SourceNominalId,
    arguments: Vec<SignatureTypeKey>,
) -> SignatureTypeKey {
    match id {
        SourceNominalId::Concrete(id) => {
            assert!(arguments.is_empty());
            SignatureTypeKey::Nominal(id)
        }
        SourceNominalId::GenericTemplate(origin) => SignatureTypeKey::NominalApplication {
            origin,
            arguments: scoop_identity::NonEmptyVec::new(arguments).unwrap(),
        },
    }
}

pub(in super::super) fn callable(
    record: &CallableDeclarationRecordV1,
    visibility: Visibility,
    modality: Modality,
    slots: CanonicalPersistentIdsV1<PersistentDispatchSlotId>,
) -> CallableDeclarationRecordV1 {
    CallableDeclarationRecordV1::try_new(
        record.declaration(),
        record.owner(),
        record.type_parameters().clone(),
        record.receiver().cloned(),
        record.parameters().clone(),
        record.result().clone(),
        record.effects(),
        modality,
        visibility,
        slots,
    )
    .unwrap()
}

impl Builder {
    #[allow(clippy::too_many_arguments)]
    pub fn nominal(
        &mut self,
        name: &str,
        kind: Kind,
        visibility: Visibility,
        arity: u32,
        parents: Vec<SignatureTypeKey>,
        outer: Option<SourceNominalId>,
    ) -> SourceNominalId {
        let mut owners = vec![];
        if let Some(outer) = outer {
            let parent = self
                .nominals
                .iter()
                .find(|nominal| nominal.id == outer)
                .unwrap();
            owners.extend_from_slice(parent.key.owners().owners());
            owners.push(owner_atom(outer));
        }
        let key = SourceDeclarationKey::nominal(
            site(self.cone.identity(), owners),
            CanonicalIdentifier::new(name).unwrap(),
            kind.source(),
            arity,
        );
        let id = if arity == 0 {
            SourceNominalId::Concrete(PersistentTypeId::from_source_declaration(&key).unwrap())
        } else {
            SourceNominalId::GenericTemplate(
                PersistentGenericTypeId::from_source_declaration(&key).unwrap(),
            )
        };
        self.nominals.push(Nominal {
            key,
            id,
            kind,
            visibility,
            arity,
            parents,
            members: vec![],
        });
        id
    }

    pub fn set_parents(&mut self, owner: SourceNominalId, parents: Vec<SignatureTypeKey>) {
        self.nominals
            .iter_mut()
            .find(|nominal| nominal.id == owner)
            .unwrap()
            .parents = parents;
    }
}
