use super::*;
pub(super) use crate::cross_cone_type_semantics::representation::tests::support::Fixture as SourceFixture;
use std::{cell::RefCell, collections::BTreeMap};

#[derive(Clone)]
pub(super) struct Source {
    pub key: SourceDeclarationKey,
    pub access: DeclarationAccessSourceV1,
    pub shape: NominalRepresentationShapeV1,
    pub public: Option<NominalSourceShapeV1>,
}
#[derive(Clone)]
pub(super) struct Fixture {
    pub provider: ConeIdentity,
    pub required: CanonicalPersistentIdsV1<PersistentTypeId>,
    pub sources: BTreeMap<PersistentTypeId, Source>,
    pub calls: RefCell<Vec<PersistentTypeId>>,
    pub inventory_failure: bool,
}
impl Fixture {
    pub fn new() -> Self {
        Self {
            provider: ConeIdentity::CORE,
            required: CanonicalPersistentIdsV1::empty(),
            sources: BTreeMap::new(),
            calls: RefCell::new(vec![]),
            inventory_failure: false,
        }
    }
    pub fn add(
        &mut self,
        source: &SourceFixture,
        shape: NominalRepresentationShapeV1,
        public: Option<NominalSourceShapeV1>,
    ) -> PersistentTypeId {
        let owner = source.owner();
        assert!(
            self.sources
                .insert(
                    owner,
                    Source {
                        key: source.key.clone(),
                        access: source.access.clone(),
                        shape,
                        public
                    }
                )
                .is_none()
        );
        self.required =
            CanonicalPersistentIdsV1::try_new(self.sources.keys().copied().collect()).unwrap();
        owner
    }
    pub fn table(&self) -> CanonicalNominalRepresentationSupportV1 {
        CanonicalNominalRepresentationSupportV1::try_new(
            self.sources
                .values()
                .map(|source| {
                    NominalRepresentationSupportV1::try_new(
                        &source.key,
                        source.access.clone(),
                        source.shape.clone(),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap()
    }
}
impl NominalRepresentationSemanticAuthority<&'static str> for Fixture {
    fn current_provider(&self) -> ConeIdentity {
        self.provider
    }
    fn required_representation_owners(
        &self,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentTypeId>, &'static str> {
        if self.inventory_failure {
            Err("independent inventory unavailable")
        } else {
            Ok(&self.required)
        }
    }
    fn representation_source(
        &self,
        owner: PersistentTypeId,
    ) -> Result<NominalRepresentationSourceV1<'_>, &'static str> {
        self.calls.borrow_mut().push(owner);
        let source = self
            .sources
            .get(&owner)
            .ok_or("independent source unavailable")?;
        Ok(NominalRepresentationSourceV1 {
            key: &source.key,
            access: &source.access,
            shape: &source.shape,
            public_source_shape: match &source.public {
                Some(public) => NominalRepresentationPublicSourceShapeV1::PublicSourceShape(public),
                None => NominalRepresentationPublicSourceShapeV1::NoPublicSourceShape,
            },
        })
    }
}

pub(super) fn path() -> WirePath {
    WirePath::root().field(2)
}
pub(super) fn unit() -> SignatureTypeKey {
    SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id())
}
pub(super) fn key(
    name: &str,
    kind: SourceNominalKind,
    arity: u32,
    owners: Vec<DefinitionOwnerAtom>,
) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::from_outer_to_inner(owners),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        arity,
    )
}
pub(super) fn mismatch(
    table: &CanonicalNominalRepresentationSupportV1,
    fixture: &Fixture,
    expected: NominalRepresentationSourceMismatchV1,
) {
    let error = table
        .validate_source_semantics(fixture, &path())
        .unwrap_err();
    assert!(
        matches!(error, NominalRepresentationSourceSemanticError::Record { error, .. } if error == expected),
        "{error:?}"
    );
}
