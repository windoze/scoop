use std::collections::BTreeMap;

use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DecodedPersistentId, DefinitionOrigin,
    DefinitionOwnerAtom, DefinitionOwnerChain, ExactTypeKey, GeneratedNominalKey,
    NormalizedSourcePath, PackagePath, PersistentExactTypeId, PersistentIdResolver,
    PersistentTypeId, SourceContextKey, SourceDeclarationKey, SourceDeclarationSite,
    SourceIdentity, SourceNominalKind, SourceSpan,
};

use crate::{
    DeclarationAccessSourceSemanticAuthority, DeclarationAccessSourceV1, DeclaredVisibilityV1,
    DirectClassBaseV1, ExportDefinitionSourceSemanticAuthority, ExportDefinitionSourceV1,
    NominalInheritanceEdgesV1, NominalInheritanceModalityV1, NominalInheritanceSemanticAuthority,
    NominalRepresentationShapeV1, NominalRepresentationSupportV1, SourceNominalId,
};

#[derive(Clone, Default)]
pub(in crate::cross_cone_type_semantics) struct Fixture {
    pub keys: BTreeMap<SourceNominalId, SourceDeclarationKey>,
    pub access: BTreeMap<SourceNominalId, DeclarationAccessSourceV1>,
    pub origins: BTreeMap<SourceNominalId, ExportDefinitionSourceV1>,
    pub exacts: BTreeMap<PersistentExactTypeId, ExactTypeKey>,
    pub records: BTreeMap<PersistentExactTypeId, NominalInheritanceEdgesV1>,
    pub representations: BTreeMap<PersistentTypeId, NominalRepresentationSupportV1>,
    pub generated: BTreeMap<PersistentTypeId, GeneratedNominalKey>,
}

#[derive(Clone, Copy)]
pub(in crate::cross_cone_type_semantics) struct Node {
    pub source: SourceNominalId,
    pub exact: PersistentExactTypeId,
}

impl Fixture {
    pub fn add(&mut self, name: &str, kind: SourceNominalKind, owners: &[Node]) -> Node {
        let source = SourceIdentity::new(
            ConeIdentity::CORE,
            NormalizedSourcePath::new("inheritance.scoop").unwrap(),
        )
        .unwrap();
        let context = SourceContextKey::File {
            source: source.clone(),
        };
        let origin = ExportDefinitionSourceV1::new(
            DefinitionOrigin::new(source, SourceSpan::new(0, 10).unwrap(), &context).unwrap(),
        );
        let chain: Vec<_> = owners.iter().map(|node| node.source).collect();
        let key = SourceDeclarationKey::nominal(
            site(&chain),
            CanonicalIdentifier::new(name).unwrap(),
            kind,
            0,
        );
        let owner = SourceNominalId::from_source_declaration(&key).unwrap();
        let SourceNominalId::Concrete(id) = owner else {
            unreachable!()
        };
        let exact_key = ExactTypeKey::Nominal(id);
        let exact = PersistentExactTypeId::from_key(&exact_key).unwrap();
        let modality = match kind {
            SourceNominalKind::Class => NominalInheritanceModalityV1::Open,
            SourceNominalKind::Interface => NominalInheritanceModalityV1::Interface,
            _ => NominalInheritanceModalityV1::Final,
        };
        self.keys.insert(owner, key);
        self.access.insert(
            owner,
            DeclarationAccessSourceV1::try_new(DeclaredVisibilityV1::Public, chain, origin.clone())
                .unwrap(),
        );
        self.origins.insert(owner, origin);
        self.exacts.insert(exact, exact_key);
        self.records.insert(
            exact,
            NominalInheritanceEdgesV1::try_new(
                exact,
                modality,
                DirectClassBaseV1::NoClassBase,
                vec![],
            )
            .unwrap(),
        );
        if kind == SourceNominalKind::Object {
            let generated = GeneratedNominalKey::ObjectBackingClass { object: id };
            let backing_class = PersistentTypeId::from_generated_key(&generated).unwrap();
            self.generated.insert(backing_class, generated);
            let backing_exact = ExactTypeKey::Nominal(backing_class);
            self.exacts.insert(
                PersistentExactTypeId::from_key(&backing_exact).unwrap(),
                backing_exact,
            );
            self.representations.insert(
                id,
                NominalRepresentationSupportV1::try_new(
                    &self.keys[&owner],
                    self.access[&owner].clone(),
                    NominalRepresentationShapeV1::Object {
                        backing_class,
                        declared_fields: vec![],
                    },
                )
                .unwrap(),
            );
        }
        Node {
            source: owner,
            exact,
        }
    }

    pub fn edges(&mut self, node: Node, base: Option<Node>, interfaces: &[Node]) {
        let modality = self.records[&node.exact].modality();
        self.records.insert(
            node.exact,
            NominalInheritanceEdgesV1::try_new(
                node.exact,
                modality,
                base.map_or(DirectClassBaseV1::NoClassBase, |base| {
                    DirectClassBaseV1::ClassBase { exact: base.exact }
                }),
                interfaces.iter().map(|node| node.exact).collect(),
            )
            .unwrap(),
        );
    }

    pub fn modality(&mut self, node: Node, modality: NominalInheritanceModalityV1) {
        let previous = &self.records[&node.exact];
        let record = NominalInheritanceEdgesV1::try_new(
            node.exact,
            modality,
            previous.direct_base(),
            previous.direct_interfaces().to_vec(),
        )
        .unwrap();
        self.records.insert(node.exact, record);
    }

    pub fn visibility(&mut self, node: Node, visibility: DeclaredVisibilityV1) {
        let previous = &self.access[&node.source];
        let access = DeclarationAccessSourceV1::try_new(
            visibility,
            previous.lexical_owners().to_vec(),
            previous.definition_origin().clone(),
        )
        .unwrap();
        self.access.insert(node.source, access);
    }

    pub fn member(&self, owner: Node) -> (SourceDeclarationKey, DeclarationAccessSourceV1) {
        let mut chain = self.access[&owner.source].lexical_owners().to_vec();
        chain.push(owner.source);
        let key = SourceDeclarationKey::function(
            site(&chain),
            CanonicalIdentifier::new("member").unwrap(),
            0,
            None,
            vec![],
        );
        let access = DeclarationAccessSourceV1::try_new(
            DeclaredVisibilityV1::Protected,
            chain,
            self.origins[&owner.source].clone(),
        )
        .unwrap();
        (key, access)
    }
}

pub(in crate::cross_cone_type_semantics) fn site(
    owners: &[SourceNominalId],
) -> SourceDeclarationSite {
    let owners = owners
        .iter()
        .map(|owner| match owner {
            SourceNominalId::Concrete(id) => DefinitionOwnerAtom::Type(*id),
            SourceNominalId::GenericTemplate(id) => DefinitionOwnerAtom::GenericType(*id),
        })
        .collect();
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::from_outer_to_inner(owners),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

impl NominalInheritanceSemanticAuthority<&'static str> for Fixture {
    fn exact_type_key(&self, exact: PersistentExactTypeId) -> Result<&ExactTypeKey, &'static str> {
        self.exacts.get(&exact).ok_or("unknown exact")
    }
    fn nominal_declaration_key(
        &self,
        owner: SourceNominalId,
    ) -> Result<&SourceDeclarationKey, &'static str> {
        self.keys.get(&owner).ok_or("unknown source")
    }
    fn nominal_access_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&DeclarationAccessSourceV1, &'static str> {
        self.access.get(&owner).ok_or("missing source access")
    }
    fn nominal_definition_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, &'static str> {
        self.origins.get(&owner).ok_or("missing foundation origin")
    }
    fn object_representation(
        &self,
        owner: PersistentTypeId,
    ) -> Result<&NominalRepresentationSupportV1, &'static str> {
        self.object_representation_record(owner)
    }
    fn generated_nominal_key(
        &self,
        nominal: PersistentTypeId,
    ) -> Result<&GeneratedNominalKey, &'static str> {
        self.generated
            .get(&nominal)
            .ok_or("missing generated nominal key")
    }
    fn validate_definition_source(
        &self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), &'static str> {
        if self.origins.values().any(|origin| origin == source) {
            Ok(())
        } else {
            Err("unknown foundation origin")
        }
    }
}
impl Fixture {
    fn object_representation_record(
        &self,
        owner: PersistentTypeId,
    ) -> Result<&NominalRepresentationSupportV1, &'static str> {
        self.representations
            .get(&owner)
            .ok_or("missing object representation")
    }
}
impl PersistentIdResolver<PersistentExactTypeId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentExactTypeId>,
    ) -> Result<PersistentExactTypeId, Self::Error> {
        self.exacts
            .keys()
            .copied()
            .find(|known| id.as_array() == known.as_array())
            .ok_or("unknown exact")
    }
}
impl ExportDefinitionSourceSemanticAuthority<&'static str> for Fixture {
    fn current_cone(&self) -> ConeIdentity {
        ConeIdentity::CORE
    }
    fn validate_export_definition_source(
        &mut self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), &'static str> {
        self.validate_definition_source(source)
    }
}
impl DeclarationAccessSourceSemanticAuthority<&'static str> for Fixture {
    fn nominal_declaration_key(
        &self,
        owner: SourceNominalId,
    ) -> Result<&SourceDeclarationKey, &'static str> {
        self.keys.get(&owner).ok_or("unknown source")
    }
    fn nominal_definition_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, &'static str> {
        self.origins.get(&owner).ok_or("missing foundation origin")
    }
}
