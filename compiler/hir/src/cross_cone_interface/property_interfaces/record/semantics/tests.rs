mod support;

use scoop_identity::{AccessorRole, PropertyAccessorKey, PropertyOwner, SignatureTypeKey};

use super::*;
use crate::{
    PropertyCapabilityV1, PropertyPublicAccessV1, PropertyRepresentationV1, PublicNominalKindV1,
    PublicNominalShapeV1,
};
use support::*;

#[test]
fn generic_extension_property_closes_over_its_own_binder() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();

    assert_eq!(fixture.record().validate_semantics(&mut authority), Ok(()));
}

#[test]
fn generic_nominal_property_uses_one_compressed_outer_frame() {
    let (record, mut authority) = generic_member_fixture();

    assert_eq!(record.validate_semantics(&mut authority), Ok(()));
}

#[test]
fn identity_owner_arity_and_receiver_must_match() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();
    authority.identity = PropertyDeclarationIdentityShapeV1::new(
        PublicDeclarationOwnerV1::TopLevel,
        1,
        0,
        Some(own_binder(0)),
    );
    assert!(matches!(
        fixture.record().validate_semantics(&mut authority),
        Err(PropertyInterfaceSemanticValidationError::Owner { .. })
    ));

    let mut authority = fixture.authority();
    authority.identity = PropertyDeclarationIdentityShapeV1::new(
        PublicDeclarationOwnerV1::Extension,
        2,
        0,
        Some(own_binder(0)),
    );
    assert_eq!(
        fixture.record().validate_semantics(&mut authority),
        Err(
            PropertyInterfaceSemanticValidationError::TypeParameterArity {
                expected: 2,
                actual: 1,
            }
        )
    );

    let mut authority = fixture.authority();
    authority.identity = PropertyDeclarationIdentityShapeV1::new(
        PublicDeclarationOwnerV1::Extension,
        1,
        0,
        Some(own_binder(1)),
    );
    assert!(matches!(
        fixture.record().validate_semantics(&mut authority),
        Err(PropertyInterfaceSemanticValidationError::ReceiverMismatch { .. })
    ));
}

#[test]
fn bounds_receiver_and_value_type_must_close() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();
    authority.generic_nominals.clear();
    assert!(matches!(
        fixture.record().validate_semantics(&mut authority),
        Err(PropertyInterfaceSemanticValidationError::TypeParameters(_))
    ));

    let record = fixture.record_with(
        binders_with_bound(fixture.contract, own_binder(0)),
        Some(deep_binder(0)),
        SignatureTypeKey::Nominal(fixture.result),
    );
    let mut authority = fixture.authority();
    authority.identity = PropertyDeclarationIdentityShapeV1::new(
        PublicDeclarationOwnerV1::Extension,
        1,
        0,
        Some(deep_binder(0)),
    );
    assert!(matches!(
        record.validate_semantics(&mut authority),
        Err(PropertyInterfaceSemanticValidationError::Receiver(_))
    ));

    let record = fixture.record_with(
        binders_with_bound(fixture.contract, own_binder(0)),
        Some(own_binder(0)),
        SignatureTypeKey::Nominal(fixture.missing_result()),
    );
    let mut authority = fixture.authority();
    assert!(matches!(
        record.validate_semantics(&mut authority),
        Err(PropertyInterfaceSemanticValidationError::ValueType(_))
    ));
}

#[test]
fn accessor_owner_role_and_reference_are_exact() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();
    authority.accessors.insert(
        fixture.getter,
        PropertyAccessorKey::new(fixture.declaration, AccessorRole::Setter),
    );
    assert!(matches!(
        fixture.record().validate_semantics(&mut authority),
        Err(PropertyInterfaceSemanticValidationError::AccessorRole {
            expected: AccessorRole::Getter,
            actual: AccessorRole::Setter,
            ..
        })
    ));

    let mut authority = fixture.authority();
    authority.accessors.insert(
        fixture.getter,
        PropertyAccessorKey::new(
            PropertyOwner::Property(fixture.other_property),
            AccessorRole::Getter,
        ),
    );
    assert!(matches!(
        fixture.record().validate_semantics(&mut authority),
        Err(PropertyInterfaceSemanticValidationError::AccessorOwner { .. })
    ));

    let mut authority = fixture.authority();
    authority.accessors.remove(&fixture.setter);
    assert!(matches!(
        fixture.record().validate_semantics(&mut authority),
        Err(
            PropertyInterfaceSemanticValidationError::AccessorReference {
                expected_role: AccessorRole::Setter,
                ..
            }
        )
    ));
}

#[test]
fn definition_source_shape_must_match_every_exported_fact() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();
    authority.source = PropertyDeclarationSourceShapeV1::new(
        PropertyCapabilityV1::read_only(fixture.getter),
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
    );
    assert!(matches!(
        fixture.record().validate_semantics(&mut authority),
        Err(PropertyInterfaceSemanticValidationError::Capability { .. })
    ));

    let mut authority = fixture.authority();
    authority.source = PropertyDeclarationSourceShapeV1::new(
        fixture.capability(),
        PropertyRepresentationV1::AbstractSlot,
        PropertyPublicAccessV1::DirectOnly,
    );
    assert!(matches!(
        fixture.record().validate_semantics(&mut authority),
        Err(PropertyInterfaceSemanticValidationError::Representation { .. })
    ));

    let mut authority = fixture.authority();
    authority.source = PropertyDeclarationSourceShapeV1::new(
        fixture.capability(),
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::PublicSlot,
    );
    assert!(matches!(
        fixture.record().validate_semantics(&mut authority),
        Err(PropertyInterfaceSemanticValidationError::Access { .. })
    ));
}

#[test]
fn const_member_requires_an_object_owner_and_allows_host_parameters() {
    let (record, mut authority, object) = const_object_fixture();
    assert_eq!(record.validate_semantics(&mut authority), Ok(()));

    authority.concrete_nominals.insert(
        object,
        PublicNominalShapeV1::new(PublicNominalKindV1::Class, 0),
    );
    assert_eq!(
        record.validate_semantics(&mut authority),
        Err(PropertyInterfaceSemanticValidationError::ConstOwnerKind {
            actual: PublicNominalKindV1::Class,
        })
    );

    authority.concrete_nominals.insert(
        object,
        PublicNominalShapeV1::new(PublicNominalKindV1::Object, 1),
    );
    assert_eq!(record.validate_semantics(&mut authority), Ok(()));
}
