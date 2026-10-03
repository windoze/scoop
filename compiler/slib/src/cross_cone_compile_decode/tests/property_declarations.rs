use super::*;
use scoop_hir::{
    CallableDeclarationRecordV1, CanonicalNestedMemberRefsV1, CanonicalNestedNominalRefsV1,
    DeclaredVisibilityV1, NestedSourceMemberRefV1, NominalDeclarationDetailsV1,
    NominalInheritanceModalityV1, PropertyAccessorsV1, PropertyDeclarationInventoryError,
    PropertyDeclarationRecordV1,
};

mod support;
use support::Fixture;

#[test]
fn ordinary_reader_preserves_private_properties_and_restricted_setter_contracts() {
    let fixture = Fixture::new();
    let bytes = fixture.artifact();
    let builtin_bytes = builtin_provider_artifact();
    let builtin = nominal_fields::front(&builtin_bytes)
        .validate_nominal_surface(vec![])
        .unwrap();
    let checked = nominal_fields::front(&bytes)
        .validate_nominal_surface(vec![])
        .unwrap()
        .validate_property_surface(vec![])
        .unwrap()
        .validate_callable_surface(vec![builtin.nominal_provider_view()])
        .unwrap();
    let properties = checked.hir_interface().property_interfaces();
    assert_eq!(properties, &fixture.properties);
    assert!(properties.get(fixture.hidden).is_none());
    assert_eq!(
        properties
            .declaration(fixture.hidden)
            .unwrap()
            .declared_visibility(),
        DeclaredVisibilityV1::Private
    );
    let callables = checked.hir_interface().callable_interfaces();
    assert_eq!(callables, &fixture.callables);
    for accessor in [fixture.hidden_getter, fixture.setter] {
        let id = CallableTemplateOrigin::Accessor(accessor);
        assert!(callables.get(id).is_none());
        assert_eq!(
            callables.declaration(id).unwrap().declared_visibility(),
            DeclaredVisibilityV1::Private
        );
    }
    assert_eq!(
        callables
            .declaration(CallableTemplateOrigin::Accessor(fixture.setter))
            .unwrap()
            .parameters()
            .parameters()[0]
            .name()
            .as_str(),
        "next"
    );
}

#[test]
fn ordinary_reader_requires_private_properties_and_every_restricted_accessor() {
    let mut fixture = Fixture::new();
    fixture.properties =
        CanonicalPropertyInterfacesV1::try_new(fixture.properties.records().to_vec()).unwrap();
    let bytes = fixture.artifact();
    assert!(
        matches!(declaration_front(&bytes).validate_internal_hir_closures(),
        Err(CrossConeHirInternalClosureError::Interface(CrossConeHirInternalClosureValidationError::PropertyDeclarations(PropertyDeclarationInventoryError::Missing(id)))) if id == fixture.hidden)
    );
    for setter in [false, true] {
        let mut fixture = Fixture::new();
        let missing = CallableTemplateOrigin::Accessor(if setter {
            fixture.setter
        } else {
            fixture.hidden_getter
        });
        fixture.callables = CanonicalCallableInterfacesV1::with_support(
            fixture.callables.records().to_vec(),
            fixture
                .callables
                .support_records()
                .iter()
                .filter(|r| r.declaration() != missing)
                .cloned()
                .collect(),
        )
        .unwrap();
        let bytes = fixture.artifact();
        assert!(
            matches!(declaration_front(&bytes).validate_internal_hir_closures(),
            Err(CrossConeHirInternalClosureError::Interface(CrossConeHirInternalClosureValidationError::CallableDeclarations(scoop_hir::CallableDeclarationInventoryError::Missing(id)))) if id == missing)
        );
    }
}

#[test]
fn ordinary_reader_rejects_private_property_owner_and_value_disagreement() {
    let mut fixture = Fixture::new();
    let original = fixture.properties.declaration(fixture.hidden).unwrap();
    fixture.change_hidden(
        PublicDeclarationOwnerV1::TopLevel,
        original.value_type().clone(),
    );
    let bytes = fixture.artifact();
    assert!(
        matches!(declaration_front(&bytes).validate_internal_hir_closures(),
        Err(CrossConeHirInternalClosureError::Interface(CrossConeHirInternalClosureValidationError::PropertyDeclarations(PropertyDeclarationInventoryError::Owner { declaration, .. }))) if declaration == fixture.hidden)
    );
    let mut fixture = Fixture::new();
    let owner = fixture
        .properties
        .declaration(fixture.hidden)
        .unwrap()
        .owner();
    fixture.change_hidden(
        owner,
        SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
    );
    let bytes = fixture.artifact();
    assert!(
        matches!(declaration_front(&bytes).validate_internal_hir_closures(),
        Err(CrossConeHirInternalClosureError::Interface(CrossConeHirInternalClosureValidationError::PropertyAccessors(PropertyAccessorClosureValidationError::Result { accessor, .. }))) if accessor == fixture.hidden_getter)
    );
}

#[test]
fn ordinary_reader_checks_restricted_accessor_signatures_and_visibility() {
    for visibility in [false, true] {
        let mut fixture = Fixture::new();
        let id = CallableTemplateOrigin::Accessor(if visibility {
            fixture.hidden_getter
        } else {
            fixture.setter
        });
        let support = fixture
            .callables
            .support_records()
            .iter()
            .map(|record| {
                if record.declaration() != id {
                    return record.clone();
                }
                CallableDeclarationRecordV1::try_new(
                    record.declaration(),
                    record.owner(),
                    record.type_parameters().clone(),
                    record.receiver().cloned(),
                    record.parameters().clone(),
                    if visibility {
                        record.result().clone()
                    } else {
                        fixture
                            .properties
                            .declaration(fixture.hidden)
                            .unwrap()
                            .value_type()
                            .clone()
                    },
                    record.effects(),
                    record.modality(),
                    if visibility {
                        DeclaredVisibilityV1::Internal
                    } else {
                        record.declared_visibility()
                    },
                    record.slot_relations().clone(),
                )
                .unwrap()
            })
            .collect();
        fixture.callables = CanonicalCallableInterfacesV1::with_support(
            fixture.callables.records().to_vec(),
            support,
        )
        .unwrap();
        let bytes = fixture.artifact();
        let Err(CrossConeHirInternalClosureError::Interface(
            CrossConeHirInternalClosureValidationError::PropertyAccessors(error),
        )) = declaration_front(&bytes).validate_internal_hir_closures()
        else {
            panic!("invalid accessor contract must be rejected")
        };
        if visibility {
            assert!(
                matches!(error, PropertyAccessorClosureValidationError::Visibility { accessor, .. } if accessor == fixture.hidden_getter)
            );
        } else {
            assert!(
                matches!(error, PropertyAccessorClosureValidationError::Result { accessor, .. } if accessor == fixture.setter)
            );
        }
    }
}

#[test]
fn ordinary_reader_checks_property_and_accessor_definition_origins() {
    let mut fixture = Fixture::new();
    let PropertyOwner::Property(property) = fixture.hidden else {
        panic!("ordinary fixture")
    };
    let subject = DefinitionOriginSubject::Property(property);
    fixture.move_origin(subject);
    fixture.move_origin(DefinitionOriginSubject::PropertyAccessor(
        fixture.hidden_getter,
    ));
    let bytes = fixture.artifact();
    assert!(
        matches!(nominal_fields::front(&bytes).validate_nominal_surface(vec![]).unwrap().validate_property_surface(vec![]),
        Err(CrossConeHirPropertySurfaceError::Declarations(CrossConeHirNominalAuthorityError::DeclarationOrigin { subject: actual, .. })) if actual == subject)
    );
    for public in [false, true] {
        let mut fixture = Fixture::new();
        let id = if public {
            fixture.callables.records()[0].declaration()
        } else {
            CallableTemplateOrigin::Accessor(fixture.setter)
        };
        let CallableTemplateOrigin::Accessor(accessor) = id else {
            panic!("accessor fixture")
        };
        fixture.move_origin(DefinitionOriginSubject::PropertyAccessor(accessor));
        let bytes = fixture.artifact();
        let mut decoded = open_graph(&bytes)
            .decode_cross_cone_hir_front_sections()
            .unwrap();
        let identities = decoded
            .validate_foundation_identities(std::iter::empty())
            .unwrap();
        assert!(matches!(decoded.validate_foundation_structure(identities),
            Err(crate::StrongProfileFoundationError::HirStructure(scoop_hir::HirFoundationValidationError::Origin(scoop_hir::DefinitionOriginValidationError::SourceMismatch { subject: DefinitionOriginSubject::PropertyAccessor(actual), expected, actual: source })))
            if actual == accessor && expected.logical_path().as_str() == "src/Container.scoop" && source.logical_path().as_str() == "src/Other.scoop"));
    }
}
