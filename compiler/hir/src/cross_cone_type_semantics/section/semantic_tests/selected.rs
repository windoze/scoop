use super::*;

#[test]
fn terminal_selection_retains_provider_and_deduplicates_targets_after_each_source_replay() {
    let mut provider = Fixture::new(ConeIdentity::CORE);
    let owner = provider.add("Provider", true);
    let provider_wire = provider.section(vec![]);
    let public = public();
    let terminal = check(&provider, &provider_wire, &public, &[], &Uses::default()).unwrap();
    let mut consumer = Fixture::new(ConeIdentity::SINGLE_FILE);
    consumer.import(&provider);
    let request = SelectedExternalTypeUseV1::new(
        provider.provider,
        SelectedTypeUseV1::Representation { exact: owner.exact },
    );
    let wire = consumer.section(vec![request]);
    let uses = Uses::new(&[request, request]);
    let checked = check(&consumer, &wire, &public, &[&terminal], &uses).unwrap();
    assert_eq!(uses.root_calls.get(), 2);
    assert_eq!(checked.selected().len(), 1);
    assert!(std::ptr::eq(checked.selected()[0].terminal(), &terminal));
    assert!(
        checked
            .section()
            .representation_support()
            .records()
            .is_empty()
    );
    let CheckedTypeSelectionDefinitionV1::SourceNominal { inheritance, .. } =
        checked.selected()[0].target().definition()
    else {
        panic!("a source class must retain its inheritance record");
    };
    assert_eq!(inheritance.owner(), owner.exact);
}

#[test]
fn repeated_target_does_not_hide_a_denied_second_source_use() {
    let mut provider = Fixture::new(ConeIdentity::CORE);
    let owner = provider.add("Provider", true);
    let provider_wire = provider.section(vec![]);
    let public = public();
    let terminal = check(&provider, &provider_wire, &public, &[], &Uses::default()).unwrap();
    let mut consumer = Fixture::new(ConeIdentity::SINGLE_FILE);
    consumer.import(&provider);
    let request = SelectedExternalTypeUseV1::new(
        provider.provider,
        SelectedTypeUseV1::Signature { exact: owner.exact },
    );
    let wire = consumer.section(vec![request]);
    let mut uses = Uses::new(&[request, request]);
    uses.roots[1].permitted = false;
    assert!(
        matches!(check(&consumer, &wire, &public, &[&terminal], &uses), Err(TypeSectionSemanticValidationError::Selected(error))
        if matches!(*error, TypeSelectionValidationError::Source("source access denied")))
    );
    assert_eq!(uses.root_calls.get(), 2);
}

#[test]
fn selected_inventory_is_exact_and_source_only_targets_remain_ineligible() {
    let mut provider = Fixture::new(ConeIdentity::CORE);
    let owner = provider.add("SourceOnly", false);
    let provider_wire = provider.section(vec![]);
    let public = public();
    let terminal = check(&provider, &provider_wire, &public, &[], &Uses::default()).unwrap();
    let mut consumer = Fixture::new(ConeIdentity::SINGLE_FILE);
    consumer.import(&provider);
    let request = SelectedExternalTypeUseV1::new(
        provider.provider,
        SelectedTypeUseV1::Representation { exact: owner.exact },
    );
    let wire = consumer.section(vec![request]);
    assert!(
        check(
            &consumer,
            &wire,
            &public,
            &[&terminal],
            &Uses::new(&[request])
        )
        .is_err()
    );
    assert!(
        matches!(check(&consumer, &wire, &public, &[&terminal], &Uses::default()), Err(TypeSectionSemanticValidationError::Selected(error))
        if matches!(*error, TypeSelectionValidationError::Inventory))
    );
}
