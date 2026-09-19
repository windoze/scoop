use super::*;
mod callables;
mod inheritance;
mod section;

pub(super) struct Fixture {
    pub provider: ConeIdentity,
    pub source: SourceFixture,
    pub roots: Vec<SourceNominalId>,
    pub edges: Vec<NominalInheritanceEdgesV1>,
    pub representations: CanonicalPersistentIdsV1<PersistentTypeId>,
    pub facts: CanonicalPersistentIdsV1<PersistentExactTypeId>,
    pub foreign: Vec<TypeSectionDependencyFactV1>,
    pub shapes: BTreeMap<PersistentExactTypeId, ExactTypeFactShapeV1>,
    pub declarations: Vec<ProtectedDeclarationInterfaceV1>,
    pub protocols: Vec<ProtectedCallableSourceInterfaceV1>,
    pub slots: BTreeMap<PersistentExactTypeId, Vec<InheritanceSlotContractV1>>,
}
impl Fixture {
    pub fn new(provider: ConeIdentity) -> Self {
        Self {
            provider,
            source: SourceFixture::default(),
            roots: vec![],
            edges: vec![],
            representations: CanonicalPersistentIdsV1::empty(),
            facts: CanonicalPersistentIdsV1::empty(),
            foreign: vec![],
            shapes: BTreeMap::new(),
            declarations: vec![],
            protocols: vec![],
            slots: BTreeMap::new(),
        }
    }
    pub fn add(&mut self, name: &str, active: bool) -> Node {
        self.add_kind(name, SourceNominalKind::Class, active)
    }
    pub fn add_kind(&mut self, name: &str, kind: SourceNominalKind, active: bool) -> Node {
        let key = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                self.provider,
                PackagePath::root(),
                DefinitionOwnerChain::from_outer_to_inner(vec![]),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new(name).unwrap(),
            kind,
            0,
        );
        let owner = PersistentTypeId::from_source_declaration(&key).unwrap();
        let source = SourceNominalId::Concrete(owner);
        let exact_key = ExactTypeKey::Nominal(owner);
        let exact = PersistentExactTypeId::from_key(&exact_key).unwrap();
        let file = if self.provider == ConeIdentity::SINGLE_FILE {
            SourceIdentity::single_file()
        } else {
            SourceIdentity::new(
                self.provider,
                NormalizedSourcePath::new("section.scoop").unwrap(),
            )
            .unwrap()
        };
        let context = SourceContextKey::File {
            source: file.clone(),
        };
        let origin = ExportDefinitionSourceV1::new(
            scoop_identity::DefinitionOrigin::new(
                file,
                SourceSpan::new(
                    1 + self.roots.len() as u64 * 10,
                    8 + self.roots.len() as u64 * 10,
                )
                .unwrap(),
                &context,
            )
            .unwrap(),
        );
        let access = DeclarationAccessSourceV1::try_new(
            DeclaredVisibilityV1::Public,
            vec![],
            origin.clone(),
        )
        .unwrap();
        let edges = NominalInheritanceEdgesV1::try_new(
            exact,
            if kind == SourceNominalKind::Object {
                NominalInheritanceModalityV1::Final
            } else {
                NominalInheritanceModalityV1::Open
            },
            DirectClassBaseV1::NoClassBase,
            vec![],
        )
        .unwrap();
        self.source.graph.keys.insert(source, key.clone());
        self.source.graph.access.insert(source, access.clone());
        self.source.graph.origins.insert(source, origin);
        self.source.graph.exacts.insert(exact, exact_key);
        self.source.graph.records.insert(exact, edges.clone());
        self.roots.push(source);
        self.roots.sort();
        if active {
            self.edges.push(edges);
            self.edges.sort_by_key(NominalInheritanceEdgesV1::owner);
            self.source.graph.representations.insert(
                owner,
                NominalRepresentationSupportV1::try_new(
                    &key,
                    access,
                    NominalRepresentationShapeV1::Class {
                        base: OptionalSignatureType::Absent,
                        declared_fields: vec![],
                    },
                )
                .unwrap(),
            );
            self.representations = CanonicalPersistentIdsV1::try_new(
                self.representations
                    .values()
                    .iter()
                    .copied()
                    .chain(std::iter::once(owner))
                    .collect(),
            )
            .unwrap();
            self.shapes.insert(exact, ExactTypeFactShapeV1::Reference);
            self.facts =
                CanonicalPersistentIdsV1::try_new(self.shapes.keys().copied().collect()).unwrap();
            let inventory = &mut self.source.inheritance_interfaces;
            inventory.owners = CanonicalPersistentIdsV1::try_new(
                self.edges
                    .iter()
                    .map(NominalInheritanceEdgesV1::owner)
                    .collect(),
            )
            .unwrap();
            inventory.schemas.insert(
                exact,
                CanonicalInheritanceSlotSchemasV1::try_new(vec![
                    InheritanceSlotSchemaV1::try_new(
                        InheritanceSlotSchemaRoleV1::ClassVtable,
                        vec![],
                    )
                    .unwrap(),
                ])
                .unwrap(),
            );
            inventory
                .constructors
                .insert(exact, CanonicalPersistentIdsV1::empty());
            inventory
                .members
                .insert(exact, CanonicalProtectedDeclarationRefsV1::default());
        }
        Node { source, exact }
    }
    pub fn import(&mut self, dependency: &Fixture) {
        let dst = &mut self.source.graph;
        let src = &dependency.source.graph;
        dst.keys.extend(src.keys.clone());
        dst.access.extend(src.access.clone());
        dst.origins.extend(src.origins.clone());
        dst.exacts.extend(src.exacts.clone());
        dst.records.extend(src.records.clone());
        dst.representations.extend(src.representations.clone());
        dst.generated.extend(src.generated.clone());
        self.source
            .inheritance_interfaces
            .schemas
            .extend(dependency.source.inheritance_interfaces.schemas.clone());
    }
}
