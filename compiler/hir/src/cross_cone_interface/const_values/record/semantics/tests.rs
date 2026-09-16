use scoop_identity::{
    ConeIdentity, DeclarationScope, PersistentPropertyId, PropertyOwner, SignatureTypeKey,
    SourceDeclarationKey, SourceDeclarationKind,
};

use super::*;
use crate::{CanonicalConstValueKindV1, CanonicalConstValueV1, PropertyRepresentationV1};

mod support;

use support::*;

#[test]
fn validates_every_value_kind_against_its_exact_core_nominal() {
    let fixture = Fixture::new();

    for (value, kind) in value_cases() {
        let value_type = fixture.value_type(kind);
        let record = fixture.record_with(value, value_type, fixture.origin.clone());
        let mut authority = fixture.authority();
        authority.interface = Some(property_interface(
            fixture.property,
            value_type,
            PropertyRepresentationV1::Const,
        ));

        assert_eq!(record.value().kind(), kind);
        assert_eq!(record.validate_semantics(&mut authority), Ok(()));
    }
}

#[test]
fn reports_missing_declaration_authority() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();
    authority.source_property = fixture.other_property;

    assert_eq!(
        fixture.record().validate_semantics(&mut authority),
        Err(ExportConstValueSemanticValidationError::Declaration(
            TestAuthorityError::Declaration(fixture.property)
        ))
    );
}

#[test]
fn rejects_wrong_declaration_kind_and_same_kind_identity() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();
    authority.source.declaration =
        SourceDeclarationKey::type_alias(current_site(), identifier("Answer"));
    assert_eq!(
        fixture.record().validate_semantics(&mut authority),
        Err(ExportConstValueSemanticValidationError::DeclarationKind {
            actual: SourceDeclarationKind::TypeAlias,
        })
    );

    let mut authority = fixture.authority();
    authority.source.declaration = fixture.other_property_key.clone();
    assert_eq!(
        fixture.record().validate_semantics(&mut authority),
        Err(
            ExportConstValueSemanticValidationError::DeclarationIdentityMismatch {
                expected: fixture.property,
                actual: fixture.other_property,
            }
        )
    );
}

#[test]
fn rejects_foreign_and_source_scoped_property_identities() {
    let fixture = Fixture::new();
    let foreign = foreign_cone();
    let foreign_key = property_key("Answer", site(foreign, DeclarationScope::ConeWide));
    let foreign_property = PersistentPropertyId::from_source_declaration(&foreign_key).unwrap();
    let foreign_record = record_for(
        foreign_property,
        fixture.value_type(CanonicalConstValueKindV1::Boolean),
        CanonicalConstValueV1::Boolean(crate::CanonicalBooleanV1::True),
        fixture.origin.clone(),
    );
    let mut authority = fixture.authority();
    authority.source_property = foreign_property;
    authority.source.declaration = foreign_key;
    authority.interface_property = foreign_property;
    authority.interface = Some(property_interface(
        foreign_property,
        fixture.value_type(CanonicalConstValueKindV1::Boolean),
        PropertyRepresentationV1::Const,
    ));
    assert_eq!(
        foreign_record.validate_semantics(&mut authority),
        Err(ExportConstValueSemanticValidationError::DeclarationCone {
            expected: ConeIdentity::CORE,
            actual: foreign,
        })
    );

    let scoped_source = source(ConeIdentity::CORE, "src/scoped.scoop");
    let scope = DeclarationScope::SourceScoped(scoped_source.clone());
    let scoped_key = property_key("Answer", site(ConeIdentity::CORE, scope.clone()));
    let scoped_property = PersistentPropertyId::from_source_declaration(&scoped_key).unwrap();
    let scoped_record = record_for(
        scoped_property,
        fixture.value_type(CanonicalConstValueKindV1::Boolean),
        CanonicalConstValueV1::Boolean(crate::CanonicalBooleanV1::True),
        fixture.origin.clone(),
    );
    let mut authority = fixture.authority();
    authority.source_property = scoped_property;
    authority.source.declaration = scoped_key;
    authority.interface_property = scoped_property;
    authority.interface = Some(property_interface(
        scoped_property,
        fixture.value_type(CanonicalConstValueKindV1::Boolean),
        PropertyRepresentationV1::Const,
    ));
    assert_eq!(
        scoped_record.validate_semantics(&mut authority),
        Err(ExportConstValueSemanticValidationError::DeclarationScope { actual: scope })
    );
}

#[test]
fn definition_origin_must_be_current_and_match_the_foundation() {
    let fixture = Fixture::new();
    let foreign = foreign_cone();
    let mut authority = fixture.authority();
    authority.source.definition_origin =
        definition_source(source(foreign, "src/Constants.scoop"), 0, 12)
            .origin()
            .clone();
    assert_eq!(
        fixture.record().validate_semantics(&mut authority),
        Err(
            ExportConstValueSemanticValidationError::DefinitionOriginCone {
                expected: ConeIdentity::CORE,
                actual: foreign,
            }
        )
    );

    let mut authority = fixture.authority();
    authority.source.definition_origin =
        definition_source(source(ConeIdentity::CORE, "src/Constants.scoop"), 1, 12)
            .origin()
            .clone();
    assert!(matches!(
        fixture.record().validate_semantics(&mut authority),
        Err(ExportConstValueSemanticValidationError::DefinitionOriginMismatch { .. })
    ));
}

#[test]
fn property_interface_must_exist_and_name_the_exact_ordinary_property() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();
    authority.interface = None;
    assert_eq!(
        fixture.record().validate_semantics(&mut authority),
        Err(ExportConstValueSemanticValidationError::PropertyInterface(
            TestAuthorityError::Interface(fixture.property)
        ))
    );

    let mut authority = fixture.authority();
    authority.interface = Some(property_interface(
        fixture.other_property,
        fixture.value_type(CanonicalConstValueKindV1::Boolean),
        PropertyRepresentationV1::Const,
    ));
    assert_eq!(
        fixture.record().validate_semantics(&mut authority),
        Err(
            ExportConstValueSemanticValidationError::PropertyInterfaceDeclaration {
                expected: PropertyOwner::Property(fixture.property),
                actual: PropertyOwner::Property(fixture.other_property),
            }
        )
    );
}

#[test]
fn property_interface_must_be_const_with_the_exact_value_type() {
    let fixture = Fixture::new();
    let boolean = fixture.value_type(CanonicalConstValueKindV1::Boolean);
    let string = fixture.value_type(CanonicalConstValueKindV1::String);

    let mut authority = fixture.authority();
    authority.interface = Some(property_interface(
        fixture.property,
        boolean,
        PropertyRepresentationV1::RuntimeAccessor,
    ));
    assert_eq!(
        fixture.record().validate_semantics(&mut authority),
        Err(
            ExportConstValueSemanticValidationError::PropertyRepresentation {
                actual: PropertyRepresentationV1::RuntimeAccessor,
            }
        )
    );

    let mut authority = fixture.authority();
    authority.interface = Some(property_interface(
        fixture.property,
        string,
        PropertyRepresentationV1::Const,
    ));
    assert!(matches!(
        fixture.record().validate_semantics(&mut authority),
        Err(ExportConstValueSemanticValidationError::PropertyValueTypeMismatch { .. })
    ));
}

#[test]
fn value_kind_requires_available_typed_core_authority() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();
    authority
        .core_types
        .remove(&CanonicalConstValueKindV1::Boolean);

    assert_eq!(
        fixture.record().validate_semantics(&mut authority),
        Err(
            ExportConstValueSemanticValidationError::CanonicalValueType {
                kind: CanonicalConstValueKindV1::Boolean,
                error: TestAuthorityError::Core(CanonicalConstValueKindV1::Boolean),
            }
        )
    );
}

#[test]
fn user_nominal_is_rejected_even_when_the_property_interface_matches() {
    let fixture = Fixture::new();
    let record = fixture.record_with(
        CanonicalConstValueV1::Boolean(crate::CanonicalBooleanV1::False),
        fixture.user_type,
        fixture.origin.clone(),
    );
    let mut authority = fixture.authority();
    authority.interface = Some(property_interface(
        fixture.property,
        fixture.user_type,
        PropertyRepresentationV1::Const,
    ));

    assert!(matches!(
        record.validate_semantics(&mut authority),
        Err(ExportConstValueSemanticValidationError::ValueKindTypeMismatch {
            kind: CanonicalConstValueKindV1::Boolean,
            expected,
            actual,
        }) if expected == Box::new(SignatureTypeKey::Nominal(
            fixture.value_type(CanonicalConstValueKindV1::Boolean)
        )) && actual == Box::new(SignatureTypeKey::Nominal(fixture.user_type))
    ));
}
