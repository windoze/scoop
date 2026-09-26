use super::*;

fn check(
    module: Module,
    selected: SelectedExternalMirSet,
    output: bool,
) -> SingleConeStrongMirInputError {
    if output {
        match DependencyMirOutput::try_new(module, selected) {
            Err(DependencyMirOutputError::ExternalCallables(error)) => error,
            _ => panic!("invalid external selection must be rejected"),
        }
    } else {
        seal(module, &selected).err().unwrap()
    }
}

#[test]
fn both_sealers_require_complete_selection_coverage_and_consumer() {
    let fixture = Fixture::new();
    for output in [true, false] {
        let (module, _) = fixture.mixed();
        let empty = SelectedExternalMirSet::empty(ConeIdentity::SINGLE_FILE);
        assert!(matches!(
            check(module, empty, output),
            SingleConeStrongMirInputError::ExternalCallableCountMismatch {
                module: 2,
                selected: 0
            }
        ));
        let (module, _) = fixture.mixed();
        let foreign = SelectedExternalMirSet::empty(ConeIdentity::CORE);
        assert!(matches!(
            check(module, foreign, output),
            SingleConeStrongMirInputError::ForeignExternalCallableSelection { .. }
        ));
    }
}

#[test]
fn service_role_rejects_duplicate_self_import_and_receiver() {
    let fixture = Fixture::new();
    let record = fixture.cycle_record();
    let empty = || SelectedExternalMirSet::empty(ConeIdentity::SINGLE_FILE);
    let selected = empty().with_initialization_cycle(record.clone()).unwrap();
    assert!(matches!(
        selected.with_initialization_cycle(record.clone()),
        Err(crate::SelectedExternalMirSetBuildError::DuplicateInitializationCycle)
    ));
    assert!(matches!(
        SelectedExternalMirSet::empty(ConeIdentity::CORE).with_initialization_cycle(record.clone()),
        Err(
            crate::SelectedExternalMirSetBuildError::SelectedCurrentProvider {
                provider: ConeIdentity::CORE
            }
        )
    ));
    let with_receiver = SelectedDependencyMirCallableV1::try_new(
        record.provider(),
        record.declaration(),
        record.implementation(),
        ExactCallableSignature::new(
            Effect::Ordinary,
            Some(crate::core_unit_exact_type()),
            Vec::new(),
            crate::core_unit_exact_type(),
        ),
    )
    .unwrap();
    assert!(matches!(
        empty().with_initialization_cycle(with_receiver),
        Err(crate::SelectedExternalMirSetBuildError::InvalidInitializationCycle)
    ));
}

#[test]
fn both_sealers_reject_duplicate_implementations_across_providers() {
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
    for output in [true, false] {
        let (mut module, _) = fixture.mixed();
        let selected = SelectedExternalMirSet::try_from_callables(
            ConeIdentity::SINGLE_FILE,
            vec![ordinary.clone()],
        )
        .unwrap()
        .with_initialization_cycle(record.clone())
        .unwrap();
        let ordinary = selected
            .callable_for(other_provider, record.declaration())
            .unwrap();
        let cycle = selected.initialization_cycle().unwrap();
        for ((_, value), (id, effect)) in module
            .meta
            .external_callables
            .iter_mut()
            .zip([(ordinary, GcEffect::NoGc), (cycle, GcEffect::Managed)])
        {
            *value = selected.callable_use(id, effect).unwrap();
        }
        assert!(
            matches!(check(module, selected, output), SingleConeStrongMirInputError::DuplicateExternalImplementation { implementation } if implementation == record.implementation())
        );
    }
}
