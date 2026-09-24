use super::*;
use scoop_identity::{LocalValueKey, LocalValueSelector, PersistentLocalValueId};

#[test]
fn callable_signature_positions_roundtrip_and_reject_ambiguous_wire_shapes() {
    let fixture = Fixture::new();
    let root = fixture.site(0, vec![0]).unwrap().position().root;
    for position in [
        HirCallableTypePositionV1::Receiver,
        HirCallableTypePositionV1::Parameter(0),
        HirCallableTypePositionV1::Parameter(7),
        HirCallableTypePositionV1::Result,
    ] {
        let original = HirDependencyTypeSiteV1::CallableSignature {
            root,
            position,
            exact: fixture.unit,
        };
        let bytes = encode(&original).unwrap();
        assert_eq!(&bytes[..3], &[0xa4, 0, 2]);
        let raw: DecodedHirDependencyTypeSiteV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(
            raw.resolve(&mut fixture.graph(), &mut meter(), &WirePath::root())
                .unwrap(),
            original
        );
        for length in [3, 5] {
            let mut wrong = bytes.clone();
            wrong[0] = 0xa0 + length;
            assert!(
                decode_canonical::<DecodedHirDependencyTypeSiteV1>(&wrong, DecodeLimits::default())
                    .is_err()
            );
        }
    }
    for bytes in [vec![0xa1, 0, 4], vec![0xa2, 0, 1, 1, 0], vec![0xa1, 0, 2]] {
        assert!(
            decode_canonical::<HirCallableTypePositionV1>(&bytes, DecodeLimits::default()).is_err()
        );
    }
}

#[test]
fn local_declaration_wire_keeps_its_distinct_identity_and_rejects_unknown_values() {
    let fixture = Fixture::new();
    let root = fixture.site(0, vec![0]).unwrap().position().root;
    let local = PersistentLocalValueId::from_key(&LocalValueKey::new(
        root,
        LocalValueSelector::Parameter {
            declaration_index: 0,
        },
    ))
    .unwrap();
    let original = HirDependencyTypeSiteV1::LocalValue {
        local,
        exact: fixture.unit,
    };
    let bytes = encode(&original).unwrap();
    assert_eq!(&bytes[..3], &[0xa3, 0, 3]);
    let raw: DecodedHirDependencyTypeSiteV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&raw).unwrap(), bytes);
    assert!(matches!(
        raw.resolve(&mut fixture.graph(), &mut meter(), &WirePath::root()),
        Err(HirDependencyTypeSiteResolutionError::Identity(_))
    ));
    assert!(
        decode_canonical::<DecodedHirDependencyTypeSiteV1>(&[0xa1, 0, 6], DecodeLimits::default())
            .is_err()
    );
}

#[test]
fn storage_sites_preserve_backing_and_delegate_roles_and_require_typed_property_identity() {
    use scoop_identity::{
        CanonicalIdentifier, DeclarationScope, DefinitionOwnerChain, PackagePath,
        PersistentPropertyId, PropertyOwner, SourceDeclarationKey, SourceDeclarationSite,
    };
    let fixture = Fixture::new();
    let key = SourceDeclarationKey::property(
        SourceDeclarationSite::new(
            fixture.current,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("storage").unwrap(),
    );
    let property =
        PropertyOwner::Property(PersistentPropertyId::from_source_declaration(&key).unwrap());
    for (tag, original) in [
        (
            4,
            HirDependencyTypeSiteV1::BackingStorage {
                property,
                exact: fixture.unit,
            },
        ),
        (
            5,
            crate::HirDependencyTypeSiteV1::DelegateStorage {
                property,
                exact: fixture.unit,
            },
        ),
    ] {
        let bytes = encode(&original).unwrap();
        assert_eq!(&bytes[..3], &[0xa3, 0, tag]);
        let raw: DecodedHirDependencyTypeSiteV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(encode(&raw).unwrap(), bytes);
        assert!(matches!(
            raw.resolve(&mut fixture.graph(), &mut meter(), &WirePath::root()),
            Err(HirDependencyTypeSiteResolutionError::Identity(_))
        ));
        for length in [2, 4] {
            let mut wrong = bytes.clone();
            wrong[0] = 0xa0 + length;
            assert!(
                decode_canonical::<DecodedHirDependencyTypeSiteV1>(&wrong, DecodeLimits::default())
                    .is_err()
            );
        }
    }
}
