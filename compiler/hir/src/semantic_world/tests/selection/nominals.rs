use super::*;

#[test]
fn signatures_use_actual_support_declarations_without_exposing_support_lookup() {
    let types = ProviderFixture::with_nominals(
        coordinate("signature-types"),
        package(&["support"]),
        "Packet",
        None,
    );
    let crate::SourceNominalId::Concrete(packet) = types.outer.unwrap() else {
        panic!("the fixture declares a concrete nominal")
    };
    let factory = CallableProviderFixture::new(
        coordinate("signature-factory"),
        package(&["api"]),
        "build",
        SignatureTypeKey::Nominal(packet),
    );
    let mut session = SemanticIdentitySession::new();
    let type_foundation = import_foundation(&mut session, &types, 51);
    let factory_foundation = import_callable_foundation(&mut session, &factory, 52);
    let aliases = empty_alias_expansions();
    for include_types in [false, true] {
        let support = if include_types {
            vec![ImportedProviderInput {
                foundation: &type_foundation,
                interface: &types.interface,
                alias_expansions: &aliases,
            }]
        } else {
            Vec::new()
        };
        let world = ImportedSemanticWorld::from_dependencies(
            coordinate("signature-consumer").identity().unwrap(),
            vec![ImportedProviderInput {
                foundation: &factory_foundation,
                interface: &factory.interface,
                alias_expansions: &aliases,
            }],
            support,
        )
        .unwrap();
        assert!(world.direct_package(&package(&["support"])).is_none());
        let mut selection = world.dependency_selection_plan().unwrap();
        let candidate = callable_candidate(&world, &selection, &["api", "build"]);
        if include_types {
            assert_eq!(
                candidate.capability().unwrap().signature().result(),
                exact(packet)
            );
            let selected = selection.select_callable(candidate).unwrap();
            assert_eq!(
                selection.resolve_callable(selected).unwrap().provider(),
                factory.identity()
            );
        } else {
            assert!(candidate.capability().is_none());
            assert!(matches!(
                selection.select_callable(candidate),
                Err(ImportedDependencySelectionError::CapabilityUnavailable { .. })
            ));
        }
    }
}

#[test]
fn current_declarations_join_the_same_signature_scope() {
    let current = ProviderFixture::with_nominals(
        coordinate("signature-current"),
        package(&["current"]),
        "Packet",
        None,
    );
    let crate::SourceNominalId::Concrete(packet) = current.outer.unwrap() else {
        panic!("the fixture declares a concrete nominal")
    };
    let world = ImportedSemanticWorld::from_dependencies(
        current.coordinate.identity().unwrap(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let classifier = world
        .nominal_exact_leaf_classifier(current.interface.nominal_interfaces())
        .unwrap();
    assert_eq!(
        classifier
            .classify(&SignatureTypeKey::Nominal(packet))
            .unwrap(),
        Some(exact(packet))
    );
    assert_eq!(world.direct_provider_count(), 0);
    assert_eq!(world.support_provider_count(), 0);
}
