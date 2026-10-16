use std::sync::Arc;

use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DecodedPersistentId, DefinitionOrigin,
    DefinitionOwnerChain, NormalizedSourcePath, PackagePath, PersistentGenericTypeId,
    PersistentIdResolver, PersistentKeyResolver, PersistentSourceContextId, PersistentTypeId,
    SourceContextKey, SourceDeclarationKey, SourceDeclarationSite, SourceIdentity,
    SourceNominalKind, SourceSpan,
};
use scoop_wire::{decode_canonical, encode};

use super::*;
use crate::{ExportDefinitionSourceV1, SourceNominalId};

struct Fixture {
    owner: SourceNominalId,
    origin: ExportDefinitionSourceV1,
    context: SourceContextKey,
}

fn fixture(kind: SourceNominalKind) -> Fixture {
    let source = SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("access.scoop").unwrap(),
    )
    .unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    let origin = ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(0, 10).unwrap(), &context).unwrap(),
    );
    let site = |owners| {
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            owners,
            DeclarationScope::ConeWide,
        )
        .unwrap()
    };
    let owner_key = SourceDeclarationKey::nominal(
        site(DefinitionOwnerChain::top_level()),
        CanonicalIdentifier::new("Owner").unwrap(),
        kind,
        0,
    );
    let id = PersistentTypeId::from_source_declaration(&owner_key).unwrap();
    let owner = SourceNominalId::Concrete(id);
    Fixture {
        owner,
        origin,
        context,
    }
}

impl PersistentIdResolver<ConeIdentity> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<ConeIdentity>,
    ) -> Result<ConeIdentity, Self::Error> {
        id.verify(ConeIdentity::CORE).map_err(|_| "foreign Cone")
    }
}
impl PersistentIdResolver<PersistentTypeId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<PersistentTypeId, Self::Error> {
        let SourceNominalId::Concrete(expected) = self.owner else {
            unreachable!()
        };
        id.verify(expected).map_err(|_| "unknown nominal")
    }
}
impl PersistentIdResolver<PersistentGenericTypeId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        _: DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<PersistentGenericTypeId, Self::Error> {
        Err("no generic nominal in this fixture")
    }
}
impl PersistentKeyResolver<PersistentSourceContextId, SourceContextKey> for Fixture {
    type Error = &'static str;
    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentSourceContextId>,
    ) -> Result<Arc<SourceContextKey>, Self::Error> {
        id.verify(PersistentSourceContextId::from_key(&self.context).unwrap())
            .map_err(|_| "unknown context")?;
        Ok(Arc::new(self.context.clone()))
    }
}
#[test]
fn declared_visibility_and_source_use_closed_wire_and_foundation_origin() {
    let mut fixture = fixture(SourceNominalKind::Class);
    for (visibility, tag) in [
        (DeclaredVisibilityV1::Public, 1),
        (DeclaredVisibilityV1::Internal, 2),
        (DeclaredVisibilityV1::Private, 3),
        (DeclaredVisibilityV1::Protected, 4),
    ] {
        assert_eq!(encode(&visibility).unwrap(), [0xa1, 0, tag]);
        assert_eq!(
            decode_canonical::<DeclaredVisibilityV1>(&[0xa1, 0, tag]).unwrap(),
            visibility
        );
    }
    let source = DeclarationAccessSourceV1::try_new(
        DeclaredVisibilityV1::Protected,
        vec![fixture.owner],
        fixture.origin.clone(),
    )
    .unwrap();
    let decoded: DecodedDeclarationAccessSourceV1 =
        decode_canonical(&encode(&source).unwrap()).unwrap();
    assert_eq!(decoded.resolve(&mut fixture).unwrap(), source);
}

#[test]
fn access_records_reject_repeated_owners_and_top_level_protected() {
    let fixture = fixture(SourceNominalKind::Struct);
    assert!(
        DeclarationAccessSourceV1::try_new(
            DeclaredVisibilityV1::Protected,
            vec![],
            fixture.origin.clone()
        )
        .is_err()
    );
    assert!(
        DeclarationAccessSourceV1::try_new(
            DeclaredVisibilityV1::Private,
            vec![fixture.owner, fixture.owner],
            fixture.origin.clone()
        )
        .is_err()
    );
}
