use super::*;

mod rejections;
mod wire;

fn request(exact: PersistentExactTypeId) -> SelectedExternalTypeUseV1 {
    SelectedExternalTypeUseV1::new(ConeIdentity::CORE, SelectedTypeUseV1::Signature { exact })
}

#[test]
fn builtins_retain_terminal_facts_without_source_records_for_all_type_only_uses() {
    for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
        let mut provider = Fixture::new(builtin.declaration_key().origin());
        let exact = provider.builtin(builtin);
        let wire = provider.section(vec![]);
        let public = public();
        let terminal = check(&provider, &wire, &public, &[], &Uses::default()).unwrap();
        let mut consumer = Fixture::new(ConeIdentity::SINGLE_FILE);
        consumer.import(&provider);
        let requests = [
            SelectedTypeUseV1::Signature { exact },
            SelectedTypeUseV1::Representation { exact },
            SelectedTypeUseV1::TypeTest { exact },
            SelectedTypeUseV1::ShapeSupport { exact },
        ]
        .map(|usage| SelectedExternalTypeUseV1::new(provider.provider, usage));
        let candidate = consumer.section(requests.to_vec());
        let uses = Uses::new(&requests);
        let checked = check(&consumer, &candidate, &public, &[&terminal], &uses).unwrap();
        assert_eq!(uses.root_calls.get(), 4);
        assert_eq!(checked.selected().len(), 4);
        for selected in checked.selected() {
            let target = selected.target();
            assert!(
                matches!(target.definition(), CheckedTypeSelectionDefinitionV1::LanguageBuiltin(value) if value == builtin)
            );
            assert!(std::ptr::eq(selected.terminal(), &terminal));
            assert!(std::ptr::eq(
                target.facts().record(),
                wire.exact_facts().get(exact).unwrap()
            ));
        }
        assert!(wire.representation_support().records().is_empty());
        assert!(wire.inheritance().records().is_empty());
        assert!(wire.definition_sources().sources().is_empty());
        assert!(candidate.exact_facts().records().is_empty());
        assert!(
            check(&provider, &wire, &public, &[], &uses)
                .unwrap()
                .selected()
                .is_empty()
        );
    }
}

#[test]
fn builtin_and_source_nominal_edges_share_closure_and_replay_each_incoming_use() {
    let mut provider = Fixture::new(ConeIdentity::CORE);
    let unit = request(provider.builtin(CoreBuiltinNominal::Unit));
    let any = request(provider.builtin(CoreBuiltinNominal::Any));
    let source = request(provider.add("Holder", true).exact);
    let wire = provider.section(vec![]);
    let public = public();
    let terminal = check(&provider, &wire, &public, &[], &Uses::default()).unwrap();
    let mut consumer = Fixture::new(ConeIdentity::SINGLE_FILE);
    consumer.import(&provider);
    let candidate = consumer.section(vec![unit, any, source]);
    let mut uses = Uses::new(&[source]);
    uses.edges = vec![
        (source, Uses::new(&[unit, any, unit]).roots),
        (any, Uses::new(&[source]).roots),
    ];
    let checked = check(&consumer, &candidate, &public, &[&terminal], &uses).unwrap();
    assert_eq!(checked.selected().len(), 3);
    assert_eq!(uses.edge_calls.get(), 4);
    assert_eq!(
        checked
            .selected()
            .iter()
            .filter(|selected| matches!(
                selected.target().definition(),
                CheckedTypeSelectionDefinitionV1::LanguageBuiltin(_)
            ))
            .count(),
        2
    );
    let missing = consumer.section(vec![source, unit]);
    assert!(
        matches!(check(&consumer, &missing, &public, &[&terminal], &uses), Err(TypeSectionSemanticValidationError::Selected(error)) if matches!(*error, TypeSelectionValidationError::Inventory))
    );
    uses.edges[0].1[2].permitted = false;
    assert!(
        matches!(check(&consumer, &candidate, &public, &[&terminal], &uses), Err(TypeSectionSemanticValidationError::Selected(error)) if matches!(*error, TypeSelectionValidationError::Source("semantic edge denied")))
    );
}
