use super::*;

#[test]
fn member_constructor_and_accessor_selection_use_distinct_checked_source_protocols() {
    let mut provider = Fixture::new(ConeIdentity::CORE);
    let owner = provider.add("Members", true);
    let function = provider.protected_function(owner);
    let constructor = provider.protected_constructor(owner);
    let getter = provider.protected_getter(owner);
    let wire = provider.section(vec![]);
    let public = public();
    let terminal = check(&provider, &wire, &public, &[], &Uses::default()).unwrap();
    assert!(
        terminal
            .sources()
            .get(CallableTemplateOrigin::Accessor(getter))
            .is_none()
    );
    let mut consumer = Fixture::new(ConeIdentity::SINGLE_FILE);
    consumer.import(&provider);
    consumer.source.accessors = provider.source.accessors.clone();
    let requests = vec![
        SelectedExternalTypeUseV1::new(
            ConeIdentity::CORE,
            SelectedTypeUseV1::Construct {
                exact: owner.exact,
                declaration: SelectedTypeConstructionV1::Constructor(constructor),
            },
        ),
        SelectedExternalTypeUseV1::new(
            ConeIdentity::CORE,
            SelectedTypeUseV1::MemberCall {
                receiver: owner.exact,
                declaration: InheritanceCallableDeclarationV1::Function(function),
            },
        ),
        SelectedExternalTypeUseV1::new(
            ConeIdentity::CORE,
            SelectedTypeUseV1::MemberCall {
                receiver: owner.exact,
                declaration: InheritanceCallableDeclarationV1::Getter(getter),
            },
        ),
    ];
    let candidate = consumer.section(requests.clone());
    assert_eq!(
        check(
            &consumer,
            &candidate,
            &public,
            &[&terminal],
            &Uses::new(&requests)
        )
        .unwrap()
        .selected()
        .len(),
        3
    );
    let wrong = SelectedExternalTypeUseV1::new(
        ConeIdentity::CORE,
        SelectedTypeUseV1::MemberCall {
            receiver: owner.exact,
            declaration: InheritanceCallableDeclarationV1::Setter(getter),
        },
    );
    let candidate = consumer.section(vec![wrong]);
    assert!(
        matches!(check(&consumer, &candidate, &public, &[&terminal], &Uses::new(&[wrong])), Err(TypeSectionSemanticValidationError::Selected(error))
        if matches!(*error, TypeSelectionValidationError::DeclarationRole))
    );
}

#[test]
fn member_receiver_must_belong_to_the_actual_declaration_ancestry() {
    let mut provider = Fixture::new(ConeIdentity::CORE);
    let owner = provider.add("Declaring", true);
    let other = provider.add("Unrelated", true);
    let function = provider.protected_function(owner);
    let wire = provider.section(vec![]);
    let public = public();
    let terminal = check(&provider, &wire, &public, &[], &Uses::default()).unwrap();
    let mut consumer = Fixture::new(ConeIdentity::SINGLE_FILE);
    consumer.import(&provider);
    let request = SelectedExternalTypeUseV1::new(
        ConeIdentity::CORE,
        SelectedTypeUseV1::MemberCall {
            receiver: other.exact,
            declaration: InheritanceCallableDeclarationV1::Function(function),
        },
    );
    let candidate = consumer.section(vec![request]);
    assert!(
        matches!(check(&consumer, &candidate, &public, &[&terminal], &Uses::new(&[request])), Err(TypeSectionSemanticValidationError::Selected(error))
        if matches!(*error, TypeSelectionValidationError::Receiver))
    );
}
