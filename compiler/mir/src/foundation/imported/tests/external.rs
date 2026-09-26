use super::*;
use crate::{
    CallableRole, SelectedDependencyMirCallableV1, SelectedExternalMirSet,
    SingleConeStrongMirInputError, StrongExternalCallableInput,
};
use scoop_identity::DependencyCallableDeclarationId;

mod fixture;
mod validation;
use fixture::{Fixture, seal};

#[test]
fn mixed_calls_preserve_both_roles_for_core_and_ordinary_providers() {
    let ordinary = scoop_identity::ConeCoordinate::new("tests", "initialization", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    for provider in [ConeIdentity::CORE, ordinary] {
        check_mixed_calls(provider);
    }
}

fn check_mixed_calls(provider: ConeIdentity) {
    let fixture = Fixture::at(provider);
    let (module, dependencies) = fixture.mixed();
    drop(fixture);
    assert_eq!(module.meta.external_callables.len(), 2);
    assert_eq!(dependencies.dependency_callables().count(), 2);
    let (ordinary, protocol) = {
        let mut callables = module.meta.external_callables.iter();
        let (ordinary, value) = callables.next().unwrap();
        assert!(matches!(
            dependencies
                .resolve_callable(value.reference())
                .unwrap()
                .role(),
            CallableRole::Ordinary
        ));
        assert_eq!(value.gc_effect(), GcEffect::NoGc);
        let (protocol, value) = callables.next().unwrap();
        assert!(matches!(
            dependencies
                .resolve_callable(value.reference())
                .unwrap()
                .role(),
            CallableRole::InitializationCycle
        ));
        assert_eq!(value.gc_effect(), GcEffect::Managed);
        (ordinary, protocol)
    };
    assert_ne!(ordinary, protocol);

    let output = DependencyMirOutput::try_new(module, dependencies).unwrap();
    let (module, dependencies) = output.into_parts();
    let input = seal(module, &dependencies).unwrap();
    let roots = input.materialization().external_callable_roots();
    assert_eq!(roots[1].callable(), protocol);
    assert_eq!(roots[0].callable(), ordinary);
    assert_eq!(roots[0].provider(), provider);
    assert_eq!(roots[1].provider(), provider);
    assert_eq!(roots[0].gc_effect(), GcEffect::NoGc);
}

#[test]
fn output_and_sealer_reject_an_unreferenced_entry_from_either_selection() {
    let fixture = Fixture::new();
    for index in 0..2 {
        for use_output in [true, false] {
            let (mut module, dependencies) = fixture.mixed();
            let function = module.functions.iter_mut().next().unwrap().1;
            function.body.blocks[function.body.entry]
                .statements
                .remove(index);
            let error = if use_output {
                match DependencyMirOutput::try_new(module, dependencies) {
                    Err(DependencyMirOutputError::ExternalCallables(error)) => error,
                    _ => panic!("the output must reject the unreferenced external use"),
                }
            } else {
                seal(module, &dependencies).err().unwrap()
            };
            assert!(matches!(error,
                SingleConeStrongMirInputError::UnreferencedExternalCallable { index: found }
                if found == index as u32));
        }
    }
}

#[test]
fn shared_selection_rejects_one_declaration_in_both_roles() {
    let fixture = Fixture::new();
    let record = fixture.cycle_record();
    let dependencies =
        SelectedExternalMirSet::try_from_callables(ConeIdentity::SINGLE_FILE, vec![record.clone()])
            .unwrap();
    assert!(matches!(
        dependencies.with_initialization_cycle(record),
        Err(crate::SelectedExternalMirSetBuildError::DuplicateCallable { .. })
    ));
}

#[test]
fn initialization_service_requires_managed_effect_in_both_sealers() {
    let fixture = Fixture::new();
    for use_output in [true, false] {
        let (mut module, selected) = fixture.mixed();
        let id = selected.initialization_cycle().unwrap();
        let (_, value) = module.meta.external_callables.iter_mut().nth(1).unwrap();
        *value = selected.callable_use(id, GcEffect::NoGc).unwrap();
        let error = if use_output {
            match DependencyMirOutput::try_new(module, selected) {
                Err(DependencyMirOutputError::ExternalCallables(error)) => error,
                _ => panic!("the output must reject a NoGc initialization service"),
            }
        } else {
            seal(module, &selected).err().unwrap()
        };
        assert!(matches!(
            error,
            SingleConeStrongMirInputError::InitializationCycleGcEffect { index: 1 }
        ));
    }
}
