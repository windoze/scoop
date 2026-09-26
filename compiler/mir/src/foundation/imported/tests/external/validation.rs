use super::*;

fn check(module: Module, selected: SelectedExternalMirSet) -> SingleConeStrongMirInputError {
    match DependencyMirOutput::try_new(module, selected) {
        Err(DependencyMirOutputError::ExternalCallables(error)) => error,
        _ => panic!("invalid external selection must be rejected"),
    }
}

#[test]
fn output_requires_complete_selection_coverage_and_consumer() {
    let fixture = Fixture::new();
    let (module, _) = fixture.mixed();
    let empty = SelectedExternalMirSet::empty(ConeIdentity::SINGLE_FILE);
    assert!(matches!(
        check(module, empty),
        SingleConeStrongMirInputError::ExternalCallableCountMismatch {
            module: 2,
            selected: 0
        }
    ));
    let (module, _) = fixture.mixed();
    let foreign = SelectedExternalMirSet::empty(ConeIdentity::CORE);
    assert!(matches!(
        check(module, foreign),
        SingleConeStrongMirInputError::ForeignExternalCallableSelection { .. }
    ));
}

#[test]
fn selection_rejects_the_current_provider() {
    let fixture = Fixture::new();
    let record = fixture.cycle_record();
    assert!(matches!(
        SelectedExternalMirSet::try_from_callables(ConeIdentity::CORE, vec![record]),
        Err(
            crate::SelectedExternalMirSetBuildError::SelectedCurrentProvider {
                provider: ConeIdentity::CORE
            }
        )
    ));
}

#[test]
fn output_rejects_duplicate_implementations_across_providers() {
    let fixture = Fixture::new();
    let record = fixture.cycle_record();
    let other_provider = scoop_identity::ConeCoordinate::new("tests", "other", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let ordinary = SelectedDependencyMirCallableV1::try_new(
        other_provider,
        record.declaration(),
        record.implementation(),
        record.signature().clone(),
    )
    .unwrap();
    let (mut module, _) = fixture.mixed();
    let selected = SelectedExternalMirSet::try_from_callables(
        ConeIdentity::SINGLE_FILE,
        vec![ordinary, record.clone()],
    )
    .unwrap();
    let ordinary = selected
        .callable_for(other_provider, record.implementation())
        .unwrap();
    let cycle = selected
        .callable_for(record.provider(), record.implementation())
        .unwrap();
    for ((_, value), (id, effect)) in module
        .meta
        .external_callables
        .iter_mut()
        .zip([(ordinary, GcEffect::NoGc), (cycle, GcEffect::Managed)])
    {
        *value = selected.callable_use(id, effect).unwrap();
    }
    assert!(
        matches!(check(module, selected), SingleConeStrongMirInputError::DuplicateExternalImplementation { implementation } if implementation == record.implementation())
    );
}
