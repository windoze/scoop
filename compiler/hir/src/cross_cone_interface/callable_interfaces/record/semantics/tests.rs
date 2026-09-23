use scoop_identity::{
    AccessorRole, CallableTemplateOrigin, PersistentPropertyAccessorId, PropertyAccessorKey,
    PropertyOwner, SignatureTypeKey,
};

use super::*;
use crate::{
    CallableInterfaceRecordBuildError, CallableInterfaceRecordV1, CallableModalityV1,
    NominalBoundSemanticError, PublicLookupAccessV1, SignatureBinderScopeError,
    SignatureTypeSemanticError, TypeParameterBinderSemanticValidationError,
    TypeParameterBoundLocation,
};

mod support;

use support::*;

#[test]
fn validates_identity_shape_and_two_frame_signature_scope() {
    let fixture = Fixture::new();
    let record = fixture.record();
    let mut authority = fixture.authority();

    assert!(record.validate_semantics(&mut authority).is_ok());
    assert_eq!(authority.identity.owner(), fixture.owner);
    assert_eq!(authority.identity.own_type_parameter_arity(), 1);
    assert_eq!(authority.identity.outer_type_parameter_arity(), 1);
    assert_eq!(authority.identity.receiver(), None);
    assert_eq!(
        authority.identity.parameter_types(),
        &[outer_binder(0), own_binder(0)]
    );
}

#[test]
fn rejects_identity_owner_arity_receiver_and_parameter_mismatches() {
    let fixture = Fixture::new();

    let mut authority = fixture.authority();
    authority.identity.owner = PublicDeclarationOwnerV1::TopLevel;
    assert_eq!(
        fixture.record().validate_semantics(&mut authority),
        Err(CallableInterfaceSemanticValidationError::Owner {
            expected: PublicDeclarationOwnerV1::TopLevel,
            actual: fixture.owner,
        })
    );

    let mut authority = fixture.authority();
    authority.identity.own_type_parameter_arity = 2;
    assert_eq!(
        fixture.record().validate_semantics(&mut authority),
        Err(
            CallableInterfaceSemanticValidationError::TypeParameterArity {
                expected: 2,
                actual: 1,
            }
        )
    );

    let mut authority = fixture.authority();
    authority.identity.receiver = Some(own_binder(0));
    assert_eq!(
        fixture.record().validate_semantics(&mut authority),
        Err(CallableInterfaceSemanticValidationError::ReceiverMismatch {
            expected: Some(Box::new(own_binder(0))),
            actual: None,
        })
    );

    let mut authority = fixture.authority();
    authority.identity.parameter_types.pop();
    assert_eq!(
        fixture.record().validate_semantics(&mut authority),
        Err(CallableInterfaceSemanticValidationError::ParameterArity {
            expected: 1,
            actual: 2,
        })
    );

    let mut authority = fixture.authority();
    authority.identity.parameter_types[1] = outer_binder(0);
    assert_eq!(
        fixture.record().validate_semantics(&mut authority),
        Err(
            CallableInterfaceSemanticValidationError::ParameterTypeMismatch {
                index: 1,
                expected: Box::new(outer_binder(0)),
                actual: Box::new(own_binder(0)),
            }
        )
    );
}

#[test]
fn validates_bounds_parameters_and_results_in_the_complete_scope() {
    let fixture = Fixture::new();
    let invalid_bound = fixture.record_with(
        binders_with_bound(fixture.contract, deep_binder(0)),
        vec![outer_binder(0), own_binder(0)],
        SignatureTypeKey::Nominal(fixture.result),
    );
    let mut authority = fixture.authority();
    assert!(matches!(
        invalid_bound.validate_semantics(&mut authority),
        Err(CallableInterfaceSemanticValidationError::TypeParameters(
            TypeParameterBinderSemanticValidationError {
                binder_index: 0,
                bound: TypeParameterBoundLocation::Interface { interface_index: 0 },
                error: NominalBoundSemanticError::Signature(
                    SignatureTypeSemanticError::BinderScope(
                        SignatureBinderScopeError::DepthOutOfRange {
                            depth: 2,
                            available_depths: 2,
                        }
                    )
                ),
            }
        ))
    ));

    let invalid_parameter = deep_binder(0);
    let invalid = fixture.record_with(
        binders_with_bound(fixture.contract, outer_binder(0)),
        vec![outer_binder(0), invalid_parameter.clone()],
        SignatureTypeKey::Nominal(fixture.result),
    );
    let mut authority = fixture.authority();
    authority.identity.parameter_types[1] = invalid_parameter;
    assert!(matches!(
        invalid.validate_semantics(&mut authority),
        Err(CallableInterfaceSemanticValidationError::Parameter {
            index: 1,
            error: SignatureTypeSemanticError::BinderScope(
                SignatureBinderScopeError::DepthOutOfRange {
                    depth: 2,
                    available_depths: 2,
                }
            ),
        })
    ));

    let missing_result = fixture.missing_result();
    let invalid = fixture.record_with(
        binders_with_bound(fixture.contract, outer_binder(0)),
        vec![outer_binder(0), own_binder(0)],
        SignatureTypeKey::Nominal(missing_result),
    );
    let mut authority = fixture.authority();
    assert!(matches!(
        invalid.validate_semantics(&mut authority),
        Err(CallableInterfaceSemanticValidationError::Result(
            SignatureTypeSemanticError::Reference(TestAuthorityError::Concrete(id))
        )) if id == missing_result
    ));
}

#[test]
fn property_accessor_borrows_the_property_binder_frame() {
    let fixture = Fixture::new();
    let property = fixture.extension_property();
    let accessor = PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(
        PropertyOwner::ExtensionProperty(property),
        AccessorRole::Getter,
    ))
    .unwrap();
    let declaration = CallableTemplateOrigin::Accessor(accessor);
    let record = extension_accessor_record(declaration).unwrap();
    let mut authority = fixture.authority_for(
        declaration,
        CallableDeclarationIdentityShapeV1::new(
            PublicDeclarationOwnerV1::Extension,
            0,
            1,
            Some(own_binder(0)),
            Vec::new(),
        ),
    );

    assert!(record.validate_semantics(&mut authority).is_ok());
}

#[test]
fn reports_missing_declaration_identity_shape() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();
    authority.declaration = CallableTemplateOrigin::Accessor(
        PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(
            PropertyOwner::ExtensionProperty(fixture.extension_property()),
            AccessorRole::Setter,
        ))
        .unwrap(),
    );

    assert!(matches!(
        fixture.record().validate_semantics(&mut authority),
        Err(CallableInterfaceSemanticValidationError::Declaration(
            TestAuthorityError::Callable(declaration)
        )) if declaration == fixture.declaration
    ));
}

fn extension_accessor_record(
    declaration: CallableTemplateOrigin,
) -> Result<CallableInterfaceRecordV1, CallableInterfaceRecordBuildError> {
    CallableInterfaceRecordV1::try_new(
        declaration,
        PublicDeclarationOwnerV1::Extension,
        empty_binders(),
        Some(own_binder(0)),
        parameters(Vec::new()),
        own_binder(0),
        scoop_effects(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
        crate::CanonicalPersistentIdsV1::empty(),
    )
}
