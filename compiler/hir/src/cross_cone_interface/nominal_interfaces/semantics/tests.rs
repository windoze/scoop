use scoop_identity::{SignatureTypeKey, SourceDeclarationKind};

use super::*;
use crate::{
    CanonicalNominalInterfacesV1, NominalBoundSemanticError,
    NominalInterfaceSetSemanticValidationError, NominalSignatureSemanticError,
    NominalSourceShapeSemanticError, NominalSourceShapeV1, SignatureTypeFormV1,
    StructSourceFieldSemanticError, StructSourceShapeV1,
    TypeParameterBinderSemanticValidationError, TypeParameterBoundLocation,
};

mod support;

use support::*;

#[test]
fn validates_declaration_binders_supertypes_and_owned_entries() {
    let fixture = Fixture::new();
    let record = fixture.record();
    let mut authority = fixture.authority();

    assert!(record.validate_semantics(&mut authority).is_ok());
}

#[test]
fn table_validates_records_and_reports_the_failing_index() {
    let fixture = Fixture::new();
    let records = CanonicalNominalInterfacesV1::try_new(vec![fixture.record()]).unwrap();
    assert!(records.validate_semantics(&mut fixture.authority()).is_ok());

    let mut authority = fixture.authority();
    authority.nominals.remove(&fixture.owner);
    assert!(matches!(
        records.validate_semantics(&mut authority),
        Err(NominalInterfaceSetSemanticValidationError::Record {
            index: 0,
            error: NominalInterfaceSemanticValidationError::Declaration(
                TestAuthorityError::Nominal(declaration)
            ),
        }) if declaration == fixture.owner
    ));
}

#[test]
fn rejects_declaration_kind_and_binder_arity_mismatches() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();
    let wrong_kind = fixture.record_with(
        PublicNominalKindV1::Struct,
        empty_binders(),
        empty_supertypes(),
        NominalSourceShapeV1::Struct(StructSourceShapeV1::try_new(Vec::new()).unwrap()),
    );
    assert!(matches!(
        wrong_kind.validate_semantics(&mut authority),
        Err(NominalInterfaceSemanticValidationError::DeclarationKind {
            expected: PublicNominalKindV1::Struct,
            actual: SourceDeclarationKind::Class,
        })
    ));

    let mut authority = fixture.authority();
    let wrong_arity = fixture.record_with(
        PublicNominalKindV1::Class,
        empty_binders(),
        empty_supertypes(),
        NominalSourceShapeV1::Class,
    );
    assert!(matches!(
        wrong_arity.validate_semantics(&mut authority),
        Err(
            NominalInterfaceSemanticValidationError::TypeParameterArity {
                expected: 1,
                actual: 0,
            }
        )
    ));
}

#[test]
fn validates_nominal_binder_bounds_through_interface_shapes() {
    let fixture = Fixture::new();
    let invalid_binders = class_bound_binders(fixture.generic_interface, binder(0));
    let record = fixture.record_with(
        PublicNominalKindV1::Class,
        invalid_binders,
        empty_supertypes(),
        NominalSourceShapeV1::Class,
    );
    let mut authority = fixture.authority();

    assert!(matches!(
        record.validate_semantics(&mut authority),
        Err(NominalInterfaceSemanticValidationError::TypeParameters(
            TypeParameterBinderSemanticValidationError {
                binder_index: 0,
                bound: TypeParameterBoundLocation::Class,
                error: NominalBoundSemanticError::Kind {
                    expected: PublicNominalKindV1::Class,
                    actual: PublicNominalKindV1::Interface,
                },
            }
        ))
    ));
}

#[test]
fn rejects_non_nominal_and_non_inheritable_supertypes() {
    let fixture = Fixture::new();
    let non_nominal = fixture.record_with(
        PublicNominalKindV1::Class,
        binders(),
        supertypes(vec![binder(0)]),
        NominalSourceShapeV1::Class,
    );
    let mut authority = fixture.authority();
    assert!(matches!(
        non_nominal.validate_semantics(&mut authority),
        Err(NominalInterfaceSemanticValidationError::ExactSupertype {
            index: 0,
            error: ExactSupertypeSemanticError::Signature(
                NominalSignatureSemanticError::NonNominal {
                    actual: SignatureTypeFormV1::Binder,
                }
            ),
        })
    ));

    let non_inheritable = fixture.record_with(
        PublicNominalKindV1::Class,
        binders(),
        supertypes(vec![SignatureTypeKey::Nominal(fixture.value_struct)]),
        NominalSourceShapeV1::Class,
    );
    let mut authority = fixture.authority();
    assert!(matches!(
        non_inheritable.validate_semantics(&mut authority),
        Err(NominalInterfaceSemanticValidationError::ExactSupertype {
            index: 0,
            error: ExactSupertypeSemanticError::InvalidTargetKind(PublicNominalKindV1::Struct),
        })
    ));

    let (generic_object, declaration, key) = generic_object_record();
    let mut authority = fixture.authority();
    authority.nominals.insert(declaration, key);
    assert!(matches!(
        generic_object.validate_semantics(&mut authority),
        Err(NominalInterfaceSemanticValidationError::ObjectTypeParameters { actual: 1 })
    ));
}

#[test]
fn enforces_single_class_inheritance_by_owner_kind() {
    let fixture = Fixture::new();
    let structure = fixture.struct_record(supertypes(vec![SignatureTypeKey::Nominal(
        fixture.base_class,
    )]));
    let mut authority = fixture.authority();
    assert!(matches!(
        structure.validate_semantics(&mut authority),
        Err(NominalInterfaceSemanticValidationError::ExactSupertype {
            index: 0,
            error: ExactSupertypeSemanticError::ClassNotAllowed {
                owner: PublicNominalKindV1::Struct,
            },
        })
    ));

    let two_classes = fixture.record_with(
        PublicNominalKindV1::Class,
        binders(),
        supertypes(vec![
            SignatureTypeKey::Nominal(fixture.base_class),
            SignatureTypeKey::Nominal(fixture.second_base_class),
        ]),
        NominalSourceShapeV1::Class,
    );
    let mut authority = fixture.authority();
    assert!(matches!(
        two_classes.validate_semantics(&mut authority),
        Err(NominalInterfaceSemanticValidationError::ExactSupertype {
            index: 1,
            error: ExactSupertypeSemanticError::MultipleClasses { first_index: 0 },
        })
    ));
}

#[test]
fn rejects_constructor_member_and_nested_binding_owner_mismatches() {
    let fixture = Fixture::new();

    let mut constructor_authority = fixture.authority();
    constructor_authority.constructor_owner = PublicDeclarationOwnerV1::TopLevel;
    assert!(matches!(
        fixture.record().validate_semantics(&mut constructor_authority),
        Err(NominalInterfaceSemanticValidationError::ConstructorOwner {
            index: 0,
            constructor,
            expected,
            actual: PublicDeclarationOwnerV1::TopLevel,
        }) if constructor == fixture.constructor && expected == fixture.owner
    ));

    let mut member_authority = fixture.authority();
    member_authority.member_owner = PublicDeclarationOwnerV1::TopLevel;
    assert!(matches!(
        fixture.record().validate_semantics(&mut member_authority),
        Err(NominalInterfaceSemanticValidationError::MemberOwner {
            index: 0,
            member,
            expected,
            actual: PublicDeclarationOwnerV1::TopLevel,
        }) if member == fixture.member && expected == fixture.owner
    ));

    let mut binding_authority = fixture.authority();
    binding_authority.nested_binding_owner = PublicDeclarationOwnerV1::TopLevel;
    assert!(matches!(
        fixture.record().validate_semantics(&mut binding_authority),
        Err(NominalInterfaceSemanticValidationError::NestedBindingOwner {
            index: 0,
            binding,
            expected,
            actual: PublicDeclarationOwnerV1::TopLevel,
        }) if binding == fixture.nested_binding && expected == fixture.owner
    ));
}

#[test]
fn forwards_source_shape_and_missing_declaration_failures() {
    let fixture = Fixture::new();
    let structure = fixture.struct_record(empty_supertypes());
    let mut shape_authority = fixture.authority();
    assert!(matches!(
        structure.validate_semantics(&mut shape_authority),
        Err(NominalInterfaceSemanticValidationError::SourceShape(
            NominalSourceShapeSemanticError::StructField {
                index: 0,
                error: StructSourceFieldSemanticError::Owner { .. },
            }
        ))
    ));

    let mut missing_authority = fixture.authority();
    missing_authority.nominals.remove(&fixture.owner);
    assert!(matches!(
        fixture.record().validate_semantics(&mut missing_authority),
        Err(NominalInterfaceSemanticValidationError::Declaration(
            TestAuthorityError::Nominal(declaration)
        )) if declaration == fixture.owner
    ));
}
