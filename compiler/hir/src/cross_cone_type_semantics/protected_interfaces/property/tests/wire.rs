use super::*;

#[test]
fn decoded_property_rejects_const_and_cross_role_accessor_aliases() {
    let (mut fixture, _, property, _, _) = setup(Some(DeclaredVisibilityV1::Private));
    let bytes = encode(&property).unwrap();
    let mut decoded = decode_canonical::<DecodedProtectedPropertyInterfaceV1>(&bytes).unwrap();
    decoded.payload.representation = PropertyRepresentationV1::Const;
    assert!(matches!(
        decoded.resolve(&mut fixture),
        Err(ProtectedPropertyResolutionError::Property(
            ProtectedPropertyBuildError::Representation
        ))
    ));
    let mut decoded = decode_canonical::<DecodedProtectedPropertyInterfaceV1>(&bytes).unwrap();
    let DecodedProtectedPropertyMutabilityV1::ReadWrite { setter, .. } =
        &mut decoded.payload.mutability
    else {
        unreachable!()
    };
    *setter = decoded.payload.getter;
    assert!(matches!(
        decoded.resolve(&mut fixture),
        Err(ProtectedPropertyResolutionError::Property(
            ProtectedPropertyBuildError::Accessor
        ))
    ));
}
