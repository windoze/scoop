use super::*;
use crate::CallableRole;

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
fn complete_role_selection_preserves_providers_and_rejects_role_duplicates() {
    let ordinary = selected(&Fixture::for_producer(ConeIdentity::CORE, "ordinary"));
    let service = selected(&Fixture::new("service"));
    let consumer = ConeIdentity::SINGLE_FILE;
    let roles = vec![
        (CallableRole::InitializationCycle, service.clone()),
        (CallableRole::Ordinary, ordinary.clone()),
    ];
    let selection = SelectedExternalLirSet::try_from_role_records(consumer, roles.clone()).unwrap();
    assert_eq!(selection.len(), 2);
    assert_eq!(selection.dependency_callables().next(), Some(&ordinary));
    let service_id = selection.initialization_cycle().unwrap();
    let selected = selection.callable(service_id).unwrap();
    assert_eq!(selected.record(), &service);
    assert_eq!(selected.role(), CallableRole::InitializationCycle);
    assert_eq!(
        selection.callable_for(service.provider(), service.bridge().declaration()),
        Some(service_id)
    );
    for records in [roles.clone(), roles.into_iter().rev().collect()] {
        let selection = SelectedExternalLirSet::try_from_role_records(consumer, records).unwrap();
        assert_eq!(selection.initialization_cycle(), Some(service_id));
    }
    let duplicate_roles = vec![
        (CallableRole::InitializationCycle, ordinary),
        (CallableRole::InitializationCycle, service.clone()),
    ];
    assert!(matches!(
        SelectedExternalLirSet::try_from_role_records(consumer, duplicate_roles),
        Err(SelectedExternalLirSetBuildError::DuplicateInitializationCycle)
    ));
    let duplicate_declarations = vec![
        (CallableRole::Ordinary, service.clone()),
        (CallableRole::InitializationCycle, service),
    ];
    assert!(matches!(
        SelectedExternalLirSet::try_from_role_records(consumer, duplicate_declarations),
        Err(SelectedExternalLirSetBuildError::DuplicateCallable { .. })
    ));
}
