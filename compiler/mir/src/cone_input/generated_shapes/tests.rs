use super::*;
use crate::{SourceExactTypeIdentities, SourceExactTypeIdentity, SourceExactTypeOrigin};
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, DeclarationScope,
    DefinitionOwnerChain, NonEmptyVec, PackagePath, SourceDeclarationSite, SourceNominalKind,
};

fn nominal(provider: ConeIdentity) -> (PersistentTypeId, SourceExactTypeIdentity) {
    let declaration = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("Payload").unwrap(),
        SourceNominalKind::Struct,
        0,
    );
    let source = PersistentTypeId::from_source_declaration(&declaration).unwrap();
    let identity = SourceExactTypeIdentity::checked(
        Type::Unit,
        CborIdentityRecord::from_key(ExactTypeKey::Nominal(source)).unwrap(),
        SourceExactTypeOrigin::Nominal(provider),
    )
    .unwrap();
    (source, identity)
}

#[test]
fn finite_helper_source_keeps_the_actual_provider() {
    let location = GeneratedExactTypeLocation::Class(crate::ClassId::from_raw(0_u32.into()));
    for provider in [
        ConeIdentity::CORE,
        ConeIdentity::SINGLE_FILE,
        ConeCoordinate::new("test", "payload-provider", "1.0.0")
            .unwrap()
            .identity()
            .unwrap(),
    ] {
        let (source, identity) = nominal(provider);
        let exact = identity.identity_record().id();
        let sources = SourceExactTypeIdentities::checked(vec![identity]).unwrap();
        assert_eq!(
            source_owner(&sources, location, exact).unwrap(),
            (source, provider)
        );
        assert!(
            matches!(source_owner(&SourceExactTypeIdentities::default(), location, exact),
            Err(ConeMirInputError::MissingGeneratedSourceExact {
                location: found_location, exact: found_exact,
            }) if found_location == location && found_exact == exact)
        );
    }
}

#[test]
fn structural_payload_cannot_claim_a_finite_nominal_helper() {
    let (_, nominal) = nominal(ConeIdentity::SINGLE_FILE);
    let identity = SourceExactTypeIdentity::checked(
        Type::Tuple(vec![Type::Unit]),
        CborIdentityRecord::from_key(ExactTypeKey::Tuple(NonEmptyVec::from_first(
            nominal.identity_record().id(),
            [],
        )))
        .unwrap(),
        SourceExactTypeOrigin::Structural,
    )
    .unwrap();
    let exact = identity.identity_record().id();
    let sources = SourceExactTypeIdentities::checked(vec![nominal, identity]).unwrap();
    let location = GeneratedExactTypeLocation::Enum(crate::EnumId::from_raw(0_u32.into()));
    assert!(matches!(source_owner(&sources, location, exact),
        Err(ConeMirInputError::InvalidGeneratedSourceOwner {
            location: found_location, exact: found_exact,
        }) if found_location == location && found_exact == exact));
}
