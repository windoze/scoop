use std::sync::Arc;

use scoop_identity::{
    CanonicalIdentifier, DeclarationScope, DecodedPersistentId, DefinitionOrigin,
    DefinitionOwnerAtom, DefinitionOwnerChain, ExactTypeKey, NormalizedSourcePath, PackagePath,
    PersistentGenericTypeId, PersistentIdResolver, PersistentKeyResolver,
    PersistentSourceContextId, PersistentTypeId, SourceContextKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind, SourceSpan,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;
use crate::{ExportDefinitionSourceSemanticAuthority, ExportDefinitionSourceV1};

struct Fixture {
    owner: SourceNominalId,
    owner_key: SourceDeclarationKey,
    member_key: SourceDeclarationKey,
    origin: ExportDefinitionSourceV1,
    context: SourceContextKey,
    exact: PersistentExactTypeId,
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
    let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(id)).unwrap();
    let member_key = SourceDeclarationKey::nominal(
        site(DefinitionOwnerChain::from_outer_to_inner(vec![
            DefinitionOwnerAtom::Type(id),
        ])),
        CanonicalIdentifier::new("Nested").unwrap(),
        SourceNominalKind::Struct,
        0,
    );
    Fixture {
        owner,
        owner_key,
        member_key,
        origin,
        context,
        exact,
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
impl PersistentIdResolver<PersistentExactTypeId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentExactTypeId>,
    ) -> Result<PersistentExactTypeId, Self::Error> {
        id.verify(self.exact).map_err(|_| "unknown exact")
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
impl ExportDefinitionSourceSemanticAuthority<&'static str> for Fixture {
    fn current_cone(&self) -> ConeIdentity {
        ConeIdentity::CORE
    }
    fn validate_export_definition_source(
        &mut self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), &'static str> {
        if source == &self.origin {
            Ok(())
        } else {
            Err("unknown source origin")
        }
    }
}
impl DeclarationAccessSourceSemanticAuthority<&'static str> for Fixture {
    fn nominal_declaration_key(
        &self,
        owner: SourceNominalId,
    ) -> Result<&SourceDeclarationKey, &'static str> {
        if owner == self.owner {
            Ok(&self.owner_key)
        } else {
            Err("unknown owner")
        }
    }
    fn nominal_definition_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, &'static str> {
        if owner == self.owner {
            Ok(&self.origin)
        } else {
            Err("unknown owner")
        }
    }
}

#[test]
fn empty_and_universal_domains_have_distinct_fixed_wire() {
    assert_eq!(
        encode(&PersistentAccessDomainV1::empty()).unwrap(),
        [0xa1, 0, 1]
    );
    assert_eq!(
        encode(&PersistentAccessDomainV1::universal()).unwrap(),
        [0xa2, 0, 2, 1, 0x80]
    );
    assert!(PersistentAccessDomainV1::empty().is_empty());
    assert!(!PersistentAccessDomainV1::empty().is_universal());
    assert!(PersistentAccessDomainV1::universal().is_universal());
    let cone = PersistentAccessConstraintV1::Cone(ConeIdentity::CORE);
    assert_eq!(
        encode(&cone).unwrap(),
        [
            b"\xa2\x00\x01\x01\x58\x20".as_slice(),
            ConeIdentity::CORE.as_array()
        ]
        .concat()
    );
}

#[test]
fn domains_round_trip_all_constraint_kinds_and_keep_purpose_fields() {
    let mut fixture = fixture(SourceNominalKind::Class);
    let constraints = vec![
        PersistentAccessConstraintV1::SubclassesOf(fixture.exact),
        PersistentAccessConstraintV1::LexicalOwner(fixture.owner),
        PersistentAccessConstraintV1::File(fixture.origin.origin().source().clone()),
        PersistentAccessConstraintV1::Cone(ConeIdentity::CORE),
    ];
    let domain = PersistentAccessDomainV1::try_from_constraints(constraints).unwrap();
    let domains = NominalAccessDomainsV1::new(
        PersistentLookupDomainV1::new(domain.clone()),
        PersistentInheritanceDomainV1::new(PersistentAccessDomainV1::empty()),
        PersistentSlotContractDomainV1::new(PersistentAccessDomainV1::universal()),
    );
    let decoded: DecodedNominalAccessDomainsV1 =
        decode_canonical(&encode(&domains).unwrap(), DecodeLimits::default()).unwrap();
    assert_eq!(decoded.resolve(&mut fixture).unwrap(), domains);
    assert_eq!(domain.intersect(&domain).unwrap(), domain);
    assert!(
        domain
            .intersect(&PersistentAccessDomainV1::empty())
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        domain
            .intersect(&PersistentAccessDomainV1::universal())
            .unwrap(),
        domain
    );
}

#[test]
fn repeated_or_reversed_constraints_are_rejected_without_repair() {
    let mut fixture = fixture(SourceNominalKind::Class);
    let cone = PersistentAccessConstraintV1::Cone(ConeIdentity::CORE);
    assert!(
        PersistentAccessDomainV1::try_from_constraints(vec![cone.clone(), cone.clone()]).is_err()
    );
    let decode_constraint = |constraint: &PersistentAccessConstraintV1| {
        decode_canonical::<DecodedPersistentAccessConstraintV1>(
            &encode(constraint).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap()
    };
    let duplicate = DecodedPersistentAccessDomainV1::Conjunction(vec![
        decode_constraint(&cone),
        decode_constraint(&cone),
    ]);
    assert!(duplicate.resolve(&mut fixture).is_err());
    let lexical = PersistentAccessConstraintV1::LexicalOwner(fixture.owner);
    let reversed = DecodedPersistentAccessDomainV1::Conjunction(vec![
        decode_constraint(&lexical),
        decode_constraint(&cone),
    ]);
    assert!(reversed.resolve(&mut fixture).is_err());
    for bytes in [
        &[0xa1, 0, 3][..],
        &[0xa2, 0, 1, 1, 0][..],
        &[0xa1, 0, 2][..],
    ] {
        assert!(
            decode_canonical::<DecodedPersistentAccessDomainV1>(bytes, DecodeLimits::default())
                .is_err()
        );
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
            decode_canonical::<DeclaredVisibilityV1>(&[0xa1, 0, tag], DecodeLimits::default())
                .unwrap(),
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
        decode_canonical(&encode(&source).unwrap(), DecodeLimits::default()).unwrap();
    assert_eq!(decoded.resolve(&mut fixture).unwrap(), source);
    let member = fixture.member_key.clone();
    assert_eq!(
        source
            .validate_for_declaration(&member, &mut fixture)
            .unwrap()
            .source(),
        &source
    );
}

#[test]
fn invalid_owner_chains_and_nonclass_protected_access_are_rejected() {
    let mut fixture = fixture(SourceNominalKind::Struct);
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
    let source = DeclarationAccessSourceV1::try_new(
        DeclaredVisibilityV1::Protected,
        vec![fixture.owner],
        fixture.origin.clone(),
    )
    .unwrap();
    let member = fixture.member_key.clone();
    assert!(matches!(
        source.validate_for_declaration(&member, &mut fixture),
        Err(DeclarationAccessSourceSemanticError::ProtectedOwnerNotClass)
    ));
    let source = DeclarationAccessSourceV1::try_new(
        DeclaredVisibilityV1::Private,
        vec![],
        fixture.origin.clone(),
    )
    .unwrap();
    assert!(matches!(
        source.validate_for_declaration(&member, &mut fixture),
        Err(DeclarationAccessSourceSemanticError::OwnerChain)
    ));
}
