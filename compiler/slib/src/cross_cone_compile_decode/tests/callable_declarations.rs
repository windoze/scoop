use super::*;
use scoop_hir::{CallableDeclarationRecordV1, DeclaredVisibilityV1};

mod support;
use support::*;

#[test]
fn ordinary_reader_validates_private_methods_and_constructors_from_shared_bytes() {
    let builtin_bytes = builtin_provider_artifact();
    let builtin = nominal_fields::front(&builtin_bytes)
        .validate_nominal_surface(vec![])
        .unwrap();
    let fixture = Fixture::new();
    let bytes = fixture.artifact(&fixture.interface);
    let checked = nominal_fields::front(&bytes)
        .validate_nominal_surface(vec![])
        .unwrap()
        .validate_property_surface(vec![])
        .unwrap()
        .validate_callable_surface(vec![builtin.nominal_provider_view()])
        .unwrap();
    let table = checked.hir_interface().callable_interfaces();
    assert_eq!(table.support_records().len(), 3);
    for declaration in [fixture.function, fixture.constructor] {
        assert!(table.get(declaration).is_none());
        assert_eq!(
            table
                .declaration(declaration)
                .unwrap()
                .declared_visibility(),
            DeclaredVisibilityV1::Private
        );
    }
}

#[test]
fn ordinary_reader_rejects_omitted_support_before_semantic_publication() {
    let fixture = Fixture::new();
    let table = fixture.interface.callable_interfaces();
    for declaration in [fixture.function, fixture.constructor] {
        let support = table
            .support_records()
            .iter()
            .filter(|r| r.declaration() != declaration)
            .cloned()
            .collect();
        let corrupt = fixture.with_callables(
            CanonicalCallableInterfacesV1::with_support(table.records().to_vec(), support).unwrap(),
        );
        let bytes = fixture.artifact(&corrupt);
        assert!(matches!(front(&bytes).validate_internal_hir_closures(),
            Err(CrossConeHirInternalClosureError::Interface(CrossConeHirInternalClosureValidationError::CallableDeclarations(
                scoop_hir::CallableDeclarationInventoryError::Missing(actual)))) if actual == declaration));
    }
}

#[test]
fn ordinary_reader_rejects_a_private_constructor_with_another_result_type() {
    let fixture = Fixture::new();
    let table = fixture.interface.callable_interfaces();
    let mut support = table.support_records().to_vec();
    let constructor = support
        .iter_mut()
        .find(|r| r.declaration() == fixture.constructor)
        .unwrap();
    *constructor = declaration(
        constructor,
        SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
    );
    let corrupt = fixture.with_callables(
        CanonicalCallableInterfacesV1::with_support(table.records().to_vec(), support).unwrap(),
    );
    let bytes = fixture.artifact(&corrupt);
    let Err(CrossConeHirCallableSurfaceError::CallableInterfaces(error)) =
        nominal_fields::front(&bytes)
            .validate_nominal_surface(vec![])
            .unwrap()
            .validate_property_surface(vec![])
            .unwrap()
            .validate_callable_surface(vec![])
    else {
        panic!("a private constructor must construct its typed owner")
    };
    assert!(matches!(
        *error,
        CallableInterfaceSetSemanticValidationError::SupportRecord {
            error: CallableInterfaceSemanticValidationError::ConstructedType { .. },
            ..
        }
    ));
}

#[test]
fn ordinary_reader_checks_support_signature_binders_without_public_lookup() {
    let fixture = Fixture::new();
    let table = fixture.interface.callable_interfaces();
    let mut support = table.support_records().to_vec();
    let function = support
        .iter_mut()
        .find(|r| r.declaration() == fixture.function)
        .unwrap();
    *function = declaration(function, SignatureTypeKey::Binder { depth: 0, index: 0 });
    let corrupt = fixture.with_callables(
        CanonicalCallableInterfacesV1::with_support(table.records().to_vec(), support).unwrap(),
    );
    let bytes = fixture.artifact(&corrupt);
    let Err(CrossConeHirCallableSurfaceError::CallableInterfaces(error)) =
        nominal_fields::front(&bytes)
            .validate_nominal_surface(vec![])
            .unwrap()
            .validate_property_surface(vec![])
            .unwrap()
            .validate_callable_surface(vec![])
    else {
        panic!("unbound private signature must be rejected")
    };
    assert!(matches!(
        *error,
        CallableInterfaceSetSemanticValidationError::SupportRecord {
            error: CallableInterfaceSemanticValidationError::Result(_),
            ..
        }
    ));
}
