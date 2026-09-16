use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    IdentityReferenceError, PackagePath, PendingIdentityValidation, PersistentGenericTypeId,
    PersistentTypeId, SourceDeclarationKey, SourceDeclarationKind, SourceDeclarationSite,
    SourceNominalKind, ValidatedIdentityGraph,
};
use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn nominal_kind_reuses_source_kind_tags() {
    let cases = [
        (PublicNominalKindV1::Class, SourceDeclarationKind::Class, 1),
        (
            PublicNominalKindV1::Interface,
            SourceDeclarationKind::Interface,
            2,
        ),
        (
            PublicNominalKindV1::Struct,
            SourceDeclarationKind::Struct,
            3,
        ),
        (PublicNominalKindV1::Enum, SourceDeclarationKind::Enum, 4),
        (
            PublicNominalKindV1::Object,
            SourceDeclarationKind::Object,
            5,
        ),
    ];

    for (public, source, byte) in cases {
        assert_eq!(public.source_kind(), source);
        assert_eq!(PublicNominalKindV1::try_from(source), Ok(public));
        assert_eq!(encode(&public).unwrap(), [byte]);
        assert_eq!(
            decode_canonical::<PublicNominalKindV1>(&[byte], DecodeLimits::default()).unwrap(),
            public
        );
    }
}

#[test]
fn unsupported_source_kinds_cannot_enter_public_nominal_surface() {
    for kind in [
        SourceDeclarationKind::AnnotationClass,
        SourceDeclarationKind::Function,
        SourceDeclarationKind::Constructor,
        SourceDeclarationKind::Property,
        SourceDeclarationKind::ExtensionProperty,
        SourceDeclarationKind::TypeAlias,
    ] {
        let error = PublicNominalKindV1::try_from(kind).unwrap_err();
        assert_eq!(error.kind(), kind);
    }
}

#[test]
fn declaration_owner_has_fixed_wire() {
    let fixture = fixture();
    let concrete =
        PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(fixture.concrete.id()));
    let generic =
        PublicDeclarationOwnerV1::Nominal(SourceNominalId::GenericTemplate(fixture.generic.id()));

    assert_eq!(
        encode(&PublicDeclarationOwnerV1::TopLevel).unwrap(),
        [0xa1, 0x00, 0x01]
    );
    assert_eq!(
        encode(&PublicDeclarationOwnerV1::Extension).unwrap(),
        [0xa1, 0x00, 0x03]
    );
    assert_eq!(
        encode(&concrete).unwrap(),
        [
            b"\xa2\x00\x02\x01\xa2\x00\x01\x01\x58\x20".as_slice(),
            fixture.concrete.id().as_array(),
        ]
        .concat()
    );
    assert_eq!(
        encode(&generic).unwrap(),
        [
            b"\xa2\x00\x02\x01\xa2\x00\x02\x01\x58\x20".as_slice(),
            fixture.generic.id().as_array(),
        ]
        .concat()
    );
}

#[test]
fn decoded_declaration_owners_resolve_typed_nominal_ids() {
    let fixture = fixture();
    let owners = [
        PublicDeclarationOwnerV1::TopLevel,
        PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(fixture.concrete.id())),
        PublicDeclarationOwnerV1::Nominal(SourceNominalId::GenericTemplate(fixture.generic.id())),
        PublicDeclarationOwnerV1::Extension,
    ];
    let mut authority = authority(&fixture);

    for owner in owners {
        let decoded = decode_owner(&owner);
        assert_eq!(decoded.resolve(&mut authority).unwrap(), owner);
    }
}

#[test]
fn decoded_declaration_owner_rejects_missing_nominal_authority() {
    let fixture = fixture();
    let owner = PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(fixture.concrete.id()));
    let decoded = decode_owner(&owner);
    let mut authority = PendingIdentityValidation::new().finish().unwrap();

    assert!(matches!(
        decoded.resolve(&mut authority),
        Err(IdentityReferenceError::Missing { .. })
    ));
}

#[test]
fn reader_rejects_unknown_tags_and_wrong_sum_lengths() {
    let unknown =
        decode_canonical::<PublicNominalKindV1>(&[0x06], DecodeLimits::default()).unwrap_err();
    assert!(matches!(
        unknown.kind(),
        WireErrorKind::UnknownTag { tag: 6 }
    ));

    let unknown = decode_canonical::<DecodedPublicDeclarationOwnerV1>(
        &[0xa1, 0x00, 0x04],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        unknown.kind(),
        WireErrorKind::UnknownTag { tag: 4 }
    ));

    let wrong_length = decode_canonical::<DecodedPublicDeclarationOwnerV1>(
        &[0xa2, 0x00, 0x01, 0x01, 0xf6],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        wrong_length.kind(),
        WireErrorKind::InvalidLength {
            expected: 1,
            actual: 2
        }
    ));
}

struct Fixture {
    concrete: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    generic: CborIdentityRecord<PersistentGenericTypeId, SourceDeclarationKey>,
}

fn fixture() -> Fixture {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    Fixture {
        concrete: CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
            site.clone(),
            CanonicalIdentifier::new("Plain").unwrap(),
            SourceNominalKind::Struct,
            0,
        ))
        .unwrap(),
        generic: CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
            site,
            CanonicalIdentifier::new("Generic").unwrap(),
            SourceNominalKind::Class,
            1,
        ))
        .unwrap(),
    }
}

fn authority(fixture: &Fixture) -> ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_external_canonical_authority(fixture.concrete.clone())
        .unwrap();
    pending
        .register_external_canonical_authority(fixture.generic.clone())
        .unwrap();
    pending.finish().unwrap()
}

fn decode_owner(owner: &PublicDeclarationOwnerV1) -> DecodedPublicDeclarationOwnerV1 {
    decode_canonical(&encode(owner).unwrap(), DecodeLimits::default()).unwrap()
}
