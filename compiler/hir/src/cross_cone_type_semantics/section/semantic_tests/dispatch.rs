use super::*;

#[test]
fn local_derived_slot_and_direct_inheritance_resolve_to_terminal_root_provider() {
    let mut provider = Fixture::new(ConeIdentity::CORE);
    let base = provider.add("Base", true);
    let slot = provider.virtual_slot(base);
    let wire = provider.section(vec![]);
    let public = public();
    let terminal = check(&provider, &wire, &public, &[], &Uses::default()).unwrap();
    let mut consumer = Fixture::new(ConeIdentity::SINGLE_FILE);
    consumer.import(&provider);
    let derived = consumer.add("Derived", true);
    consumer.inherit_slot(derived, &provider, base, slot);
    let requests = vec![
        SelectedExternalTypeUseV1::new(
            ConeIdentity::CORE,
            SelectedTypeUseV1::SlotCall {
                receiver: derived.exact,
                slot,
            },
        ),
        SelectedExternalTypeUseV1::new(
            ConeIdentity::CORE,
            SelectedTypeUseV1::Inheritance {
                derived: derived.exact,
                edge: SelectedDirectInheritanceEdgeV1::ClassBase { exact: base.exact },
            },
        ),
    ];
    let candidate = consumer.section(requests.clone());
    let checked = check(
        &consumer,
        &candidate,
        &public,
        &[&terminal],
        &Uses::new(&requests),
    )
    .unwrap();
    assert_eq!(checked.selected().len(), 2);
    for selected in checked.selected() {
        let CheckedTypeSelectionDefinitionV1::SourceNominal { inheritance, .. } =
            selected.target().definition()
        else {
            panic!("a source slot must retain its defining nominal");
        };
        assert_eq!(inheritance.owner(), base.exact);
    }
    assert_eq!(candidate.definition_sources().sources().len(), 2);
    let mut missing_origin = candidate.clone();
    missing_origin.definition_sources = CanonicalExportDefinitionSourcesV1::try_new(vec![
        consumer.source.graph.origins[&derived.source].clone(),
    ])
    .unwrap();
    assert!(
        matches!(check(&consumer, &missing_origin, &public, &[&terminal], &Uses::new(&requests)),
        Err(TypeSectionSemanticValidationError::Exports(error)) if matches!(*error, TypeSectionExportValidationError::Origins(_)))
    );
    let wrong = SelectedExternalTypeUseV1::new(
        ConeIdentity::CORE,
        SelectedTypeUseV1::Inheritance {
            derived: derived.exact,
            edge: SelectedDirectInheritanceEdgeV1::Interface { exact: base.exact },
        },
    );
    let candidate = consumer.section(vec![wrong]);
    assert!(
        matches!(check(&consumer, &candidate, &public, &[&terminal], &Uses::new(&[wrong])), Err(TypeSectionSemanticValidationError::Selected(error))
        if matches!(*error, TypeSelectionValidationError::DirectEdge))
    );
}

#[test]
fn singleton_selection_keeps_source_value_and_physical_backing_distinct() {
    let mut provider = Fixture::new(ConeIdentity::CORE);
    let (object, value, backing) = provider.object("Only");
    let (_, other_value, _) = provider.object("Other");
    let wire = provider.section(vec![]);
    let public = public();
    let terminal = check(&provider, &wire, &public, &[], &Uses::default()).unwrap();
    let mut consumer = Fixture::new(ConeIdentity::SINGLE_FILE);
    consumer.import(&provider);
    let request = |exact, value| {
        SelectedExternalTypeUseV1::new(
            ConeIdentity::CORE,
            SelectedTypeUseV1::SingletonValue { exact, value },
        )
    };
    let valid = request(object.exact, value);
    let candidate = consumer.section(vec![valid]);
    assert_eq!(
        check(
            &consumer,
            &candidate,
            &public,
            &[&terminal],
            &Uses::new(&[valid])
        )
        .unwrap()
        .selected()
        .len(),
        1
    );
    for wrong in [request(object.exact, other_value), request(backing, value)] {
        let candidate = consumer.section(vec![wrong]);
        assert!(
            matches!(check(&consumer, &candidate, &public, &[&terminal], &Uses::new(&[wrong])), Err(TypeSectionSemanticValidationError::Selected(error))
            if matches!(*error, TypeSelectionValidationError::Object))
        );
    }
}
