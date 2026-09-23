use super::*;

#[test]
fn ordinary_reader_replays_all_setter_visibility_pairs_in_a_public_class() {
    use DeclaredVisibilityV1::{Internal, Private, Protected, Public};
    let builtin_bytes = builtin_provider_artifact();
    let builtin = nominal_fields::front(&builtin_bytes)
        .validate_nominal_surface(vec![])
        .unwrap();
    for property in [Public, Internal, Private, Protected] {
        for setter in [Public, Internal, Private, Protected] {
            let mut fixture = Fixture::new();
            fixture.set_visibilities(property, setter);
            let bytes = fixture.artifact();
            let result = nominal_fields::front(&bytes)
                .validate_nominal_surface(vec![])
                .unwrap()
                .validate_property_surface(vec![])
                .unwrap()
                .validate_callable_surface(vec![builtin.nominal_provider_view()]);
            let valid = property == Public || setter == Private || property == setter;
            if valid {
                result.unwrap_or_else(|error| panic!("{property:?}/{setter:?}: {error:?}"));
            } else {
                assert!(
                    matches!(result, Err(CrossConeHirCallableSurfaceError::Declarations(
                    CrossConeHirNominalAuthorityError::CallableDeclaration { declaration, reason }
                )) if declaration == CallableTemplateOrigin::Accessor(fixture.setter)
                    && reason == "setter effective lookup domain is wider than its property domain"),
                    "{property:?}/{setter:?}"
                );
            }
        }
    }
}

#[test]
fn ordinary_reader_rejects_cyclic_source_inheritance_during_setter_domain_replay() {
    let builtin_bytes = builtin_provider_artifact();
    let builtin = nominal_fields::front(&builtin_bytes)
        .validate_nominal_surface(vec![])
        .unwrap();
    let mut fixture = Fixture::new();
    fixture.set_visibilities(
        DeclaredVisibilityV1::Protected,
        DeclaredVisibilityV1::Private,
    );
    fixture.cycle_owner_inheritance();
    let bytes = fixture.artifact();
    let result = nominal_fields::front(&bytes)
        .validate_nominal_surface(vec![])
        .unwrap()
        .validate_property_surface(vec![])
        .unwrap()
        .validate_callable_surface(vec![builtin.nominal_provider_view()]);
    assert!(
        matches!(result, Err(CrossConeHirCallableSurfaceError::Declarations(
        CrossConeHirNominalAuthorityError::NominalDeclaration { reason, .. }
    )) if reason == "cyclic source class inheritance in visibility query")
    );
}
