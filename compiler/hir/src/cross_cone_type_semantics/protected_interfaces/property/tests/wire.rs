use super::*;

#[test]
fn property_wire_preserves_both_mutability_branches_and_enforces_the_budget() {
    for visibility in [
        None,
        Some(DeclaredVisibilityV1::Private),
        Some(DeclaredVisibilityV1::Protected),
    ] {
        let (mut fixture, _, property, _, _) = setup(visibility);
        let bytes = encode(&property).unwrap();
        let decoded = decode_canonical::<DecodedProtectedPropertyInterfaceV1>(
            &bytes,
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(
            decoded.clone().resolve(&mut fixture, &mut meter()).unwrap(),
            property
        );
        let mut empty_budget = BudgetMeter::new(DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        });
        assert!(matches!(
            decoded.resolve(&mut fixture, &mut empty_budget),
            Err(ProtectedPropertyResolutionError::Access(_))
                | Err(ProtectedPropertyResolutionError::Resource(_))
        ));
    }
    assert_eq!(
        encode(&ProtectedPropertyMutabilityV1::ReadOnly).unwrap(),
        vec![0xa1, 0, 1]
    );
    assert!(
        decode_canonical::<DecodedProtectedPropertyMutabilityV1>(
            &[0xa1, 0, 3],
            DecodeLimits::default()
        )
        .is_err()
    );
    assert!(
        decode_canonical::<DecodedProtectedPropertyMutabilityV1>(
            &[0xa2, 0, 1, 1, 0],
            DecodeLimits::default()
        )
        .is_err()
    );
}

#[test]
fn decoded_property_rejects_const_and_cross_role_accessor_aliases() {
    let (mut fixture, _, property, _, _) = setup(Some(DeclaredVisibilityV1::Private));
    let bytes = encode(&property).unwrap();
    let mut decoded =
        decode_canonical::<DecodedProtectedPropertyInterfaceV1>(&bytes, DecodeLimits::default())
            .unwrap();
    decoded.payload.representation = PropertyRepresentationV1::Const;
    assert!(matches!(
        decoded.resolve(&mut fixture, &mut meter()),
        Err(ProtectedPropertyResolutionError::Property(
            ProtectedPropertyBuildError::Representation
        ))
    ));
    let mut decoded =
        decode_canonical::<DecodedProtectedPropertyInterfaceV1>(&bytes, DecodeLimits::default())
            .unwrap();
    let DecodedProtectedPropertyMutabilityV1::ReadWrite { setter, .. } =
        &mut decoded.payload.mutability
    else {
        unreachable!()
    };
    *setter = decoded.payload.getter;
    assert!(matches!(
        decoded.resolve(&mut fixture, &mut meter()),
        Err(ProtectedPropertyResolutionError::Property(
            ProtectedPropertyBuildError::Accessor
        ))
    ));
}
