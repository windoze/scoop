use super::*;

#[test]
fn validates_the_complete_local_lir_front() {
    let bytes = cross_cone_artifact(empty_cross_cone_hir_interface());
    let mut decoded = open_graph(&bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap();
    let identities = decoded
        .validate_foundation_identities(std::iter::empty())
        .unwrap();
    let validated = decoded
        .validate_foundation_structure(identities)
        .unwrap()
        .resolve_hir_interface()
        .unwrap()
        .validate_hir_production()
        .unwrap()
        .validate_internal_hir_closures()
        .unwrap()
        .validate_definition_sources(&[])
        .unwrap()
        .validate_nominal_surface(Vec::new())
        .unwrap()
        .validate_property_surface(Vec::new())
        .unwrap()
        .validate_callable_surface(Vec::new())
        .unwrap()
        .validate_type_alias_surface(Vec::new())
        .unwrap()
        .validate_source_interfaces(Vec::new())
        .unwrap()
        .validate_const_values(Vec::new())
        .unwrap()
        .validate_mir_bridge()
        .unwrap()
        .validate_lir_bridge()
        .unwrap();

    assert_eq!(validated.identity(), cone().identity());
    assert!(validated.lir_cross_cone_bridge().exports().is_empty());
    assert!(validated.lir_cross_cone_bridge().selected().is_empty());
    assert_eq!(
        validated
            .lir_strong_production()
            .image_plan()
            .cone()
            .identity(),
        cone().identity()
    );
}
