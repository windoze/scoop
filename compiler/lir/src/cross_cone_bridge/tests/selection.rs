use super::*;

fn selected(fixture: &Fixture) -> SelectedDependencyLirCallableV1 {
    SelectedDependencyLirCallableV1::new(
        fixture.producer,
        fixture.declaration,
        fixture.target,
        fixture.abi.clone(),
        CallingConvention::Cdecl,
        ExternalCallableRootPlan::NoGc,
    )
    .unwrap()
}

#[test]
fn selection_preserves_providers_order_and_unique_declarations() {
    let ordinary = selected(&Fixture::for_producer(ConeIdentity::CORE, "ordinary"));
    let service = selected(&Fixture::new("service"));
    let consumer = ConeIdentity::SINGLE_FILE;
    let records = vec![service.clone(), ordinary.clone()];
    let selection = SelectedExternalLirSet::try_from_callables(consumer, records.clone()).unwrap();
    assert_eq!(selection.len(), 2);
    assert_eq!(selection.dependency_callables().next(), Some(&ordinary));
    let service_id = selection
        .callable_for(service.provider(), service.bridge().declaration())
        .unwrap();
    let selected = selection.callable(service_id).unwrap();
    assert_eq!(selected, &service);
    for records in [records.clone(), records.into_iter().rev().collect()] {
        let selection = SelectedExternalLirSet::try_from_callables(consumer, records).unwrap();
        assert_eq!(
            selection.callable_for(service.provider(), service.bridge().declaration()),
            Some(service_id)
        );
    }
    let duplicate_declarations = vec![service.clone(), service];
    assert!(matches!(
        SelectedExternalLirSet::try_from_callables(consumer, duplicate_declarations),
        Err(SelectedExternalLirSetBuildError::DuplicateCallable { .. })
    ));
}
