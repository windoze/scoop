use super::*;

#[test]
fn builtin_selection_requires_each_source_use_and_its_real_provider() {
    for provider_id in [ConeIdentity::CORE, ConeIdentity::SINGLE_FILE] {
        let mut provider = Fixture::new(provider_id);
        let exact = provider.builtin(CoreBuiltinNominal::Unit);
        let wire = provider.section(vec![]);
        let public = public();
        let terminal = check(&provider, &wire, &public, &[], &Uses::default()).unwrap();
        let consumer_id = if provider_id == ConeIdentity::CORE {
            ConeIdentity::SINGLE_FILE
        } else {
            ConeIdentity::CORE
        };
        let mut consumer = Fixture::new(consumer_id);
        consumer.import(&provider);
        let request =
            SelectedExternalTypeUseV1::new(provider_id, SelectedTypeUseV1::Signature { exact });
        let candidate = consumer.section(vec![request]);
        let mut uses = Uses::new(&[request, request]);
        uses.roots[1].permitted = false;
        let error = check(&consumer, &candidate, &public, &[&terminal], &uses).unwrap_err();
        assert!(
            matches!(error, TypeSectionSemanticValidationError::Selected(error) if match *error {
                TypeSelectionValidationError::DeclarationOwner => provider_id != ConeIdentity::CORE && uses.root_calls.get() == 0,
                TypeSelectionValidationError::Source("source access denied") => provider_id == ConeIdentity::CORE && uses.root_calls.get() == 2,
                _ => false,
            })
        );
        assert!(check(&consumer, &candidate, &public, &[], &uses).is_err());
    }
}

#[test]
fn builtin_identity_cannot_replace_missing_or_inconsistent_facts() {
    for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
        for present in [false, true] {
            let mut provider = Fixture::new(ConeIdentity::CORE);
            let exact = provider.builtin(builtin);
            if present {
                provider.shapes.insert(
                    exact,
                    match builtin {
                        CoreBuiltinNominal::Unit => ExactTypeFactShapeV1::Reference,
                        CoreBuiltinNominal::Any => ExactTypeFactShapeV1::Unit,
                    },
                );
            } else {
                provider.shapes.clear();
                provider.facts = CanonicalPersistentIdsV1::empty();
            }
            let wire = provider.section(vec![]);
            let public = public();
            let terminal = check(&provider, &wire, &public, &[], &Uses::default()).unwrap();
            let mut consumer = Fixture::new(ConeIdentity::SINGLE_FILE);
            consumer.import(&provider);
            let request = request(exact);
            let candidate = consumer.section(vec![request]);
            let error = check(
                &consumer,
                &candidate,
                &public,
                &[&terminal],
                &Uses::new(&[request]),
            )
            .unwrap_err();
            assert!(
                matches!(error, TypeSectionSemanticValidationError::Selected(error) if match *error {
                    TypeSelectionValidationError::MissingTarget(value) => !present && value == request,
                    TypeSelectionValidationError::BuiltinFacts(value) => present && value == builtin,
                    _ => false,
                })
            );
        }
    }
}

#[test]
fn builtin_names_do_not_grant_support_to_other_source_identities() {
    let mut provider = Fixture::new(ConeIdentity::SINGLE_FILE);
    let source = provider.add("Unit", false);
    let wire = provider.section(vec![]);
    let public = public();
    let terminal = check(&provider, &wire, &public, &[], &Uses::default()).unwrap();
    let mut consumer = Fixture::new(ConeIdentity::CORE);
    consumer.import(&provider);
    let request = SelectedExternalTypeUseV1::new(
        provider.provider,
        SelectedTypeUseV1::Representation {
            exact: source.exact,
        },
    );
    let candidate = consumer.section(vec![request]);
    assert!(
        matches!(check(&consumer, &candidate, &public, &[&terminal], &Uses::new(&[request])), Err(TypeSectionSemanticValidationError::Selected(error)) if matches!(*error, TypeSelectionValidationError::MissingTarget(value) if value == request))
    );
}

#[test]
fn builtin_facts_do_not_supply_constructor_or_inheritance_declarations() {
    let mut provider = Fixture::new(ConeIdentity::CORE);
    let unit = provider.builtin(CoreBuiltinNominal::Unit);
    let any = provider.builtin(CoreBuiltinNominal::Any);
    let other = provider.add("Constructible", true);
    let constructor = provider.protected_constructor(other);
    let wire = provider.section(vec![]);
    let public = public();
    let terminal = check(&provider, &wire, &public, &[], &Uses::default()).unwrap();
    let mut consumer = Fixture::new(ConeIdentity::SINGLE_FILE);
    consumer.import(&provider);
    let derived = consumer.add("Derived", true);
    for usage in [
        SelectedTypeUseV1::Construct {
            exact: unit,
            declaration: SelectedTypeConstructionV1::Constructor(constructor),
        },
        SelectedTypeUseV1::Inheritance {
            derived: derived.exact,
            edge: SelectedDirectInheritanceEdgeV1::ClassBase { exact: any },
        },
    ] {
        let request = SelectedExternalTypeUseV1::new(provider.provider, usage);
        let candidate = consumer.section(vec![request]);
        let uses = Uses::new(&[request]);
        assert!(check(&consumer, &candidate, &public, &[&terminal], &uses).is_err());
        assert_eq!(uses.root_calls.get(), 0);
    }
}
