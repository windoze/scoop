use std::collections::BTreeMap;

use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DecodedPersistentId, DefinitionOrigin,
    DefinitionOwnerAtom, DefinitionOwnerChain, ExactTypeKey, NormalizedSourcePath, PackagePath,
    PersistentExactTypeId, PersistentIdResolver, SourceContextKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan,
};

use crate::{
    DeclarationAccessSourceV1, DeclaredVisibilityV1, DirectClassBaseV1, ExportDefinitionSourceV1,
    NominalInheritanceEdgesV1, NominalInheritanceModalityV1, SourceNominalId,
};

#[derive(Clone, Default)]
pub(in crate::cross_cone_type_semantics) struct Fixture {
    pub keys: BTreeMap<SourceNominalId, SourceDeclarationKey>,
    pub access: BTreeMap<SourceNominalId, DeclarationAccessSourceV1>,
    pub origins: BTreeMap<SourceNominalId, ExportDefinitionSourceV1>,
    pub exacts: BTreeMap<PersistentExactTypeId, ExactTypeKey>,
    pub records: BTreeMap<PersistentExactTypeId, NominalInheritanceEdgesV1>,
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
        );
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
        Node {
            source: owner,
            exact,
        }
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
