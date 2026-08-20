use super::*;
use crate::{ConeMirInputError, SelectedDependencyMirCallableV1, SelectedExternalMirSet};
use scoop_identity::DependencyCallableDeclarationId;

mod fixture;
mod validation;
use fixture::{Fixture, seal};

#[test]
fn mixed_calls_preserve_targets_and_effects_for_core_and_ordinary_providers() {
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
        assert_eq!(
            dependencies
                .resolve_callable(value.reference())
                .unwrap()
                .provider(),
            provider
        );
        assert_eq!(value.gc_effect(), GcEffect::NoGc);
        let (protocol, value) = callables.next().unwrap();
        assert_eq!(
            dependencies
                .resolve_callable(value.reference())
                .unwrap()
                .provider(),
            provider
        );
        assert_eq!(value.gc_effect(), GcEffect::Managed);
        (ordinary, protocol)
    };
    assert_ne!(ordinary, protocol);

    let output = DependencyMirOutput::try_new(module, dependencies).unwrap();
    let input = seal(output).unwrap();
    let roots = input.materialization().external_callable_roots();
    assert_eq!(roots[1].callable(), protocol);
    assert_eq!(roots[0].callable(), ordinary);
    assert_eq!(roots[0].provider(), provider);
    assert_eq!(roots[1].provider(), provider);
    assert_eq!(roots[0].gc_effect(), GcEffect::NoGc);
}

#[test]
fn unused_selected_declarations_keep_their_stable_external_indices() {
    let fixture = Fixture::new();
    for index in 0..2 {
        let (mut module, dependencies) = fixture.mixed();
        let function = module.functions.iter_mut().next().unwrap().1;
        function.body.blocks[function.body.entry]
            .statements
            .remove(index);
        let output = DependencyMirOutput::try_new(module, dependencies).unwrap();
        let input = seal(output).unwrap();
        let roots = input.materialization().external_callable_roots();
        assert_eq!(roots.len(), 2);
        for (index, root) in roots.iter().enumerate() {
            assert_eq!(root.callable().into_raw().into_u32() as usize, index);
        }
    }
}

#[test]
fn shared_selection_rejects_duplicate_declarations() {
    let fixture = Fixture::new();
    let record = fixture.cycle_record();
    assert!(matches!(
        SelectedExternalMirSet::try_from_callables(
            ConeIdentity::SINGLE_FILE,
            vec![record.clone(), record]
        ),
        Err(crate::SelectedExternalMirSetBuildError::DuplicateCallable { .. })
    ));
}
