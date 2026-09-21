use super::*;
use crate::{
    ExternalCallableSelection, SelectedDependencyMirCallableV1, SelectedDependencyMirSet,
    SingleConeStrongMirInputError, StrongImportedCoreInput, StrongImportedDependencyInput,
};
use scoop_identity::DependencyCallableDeclarationId;

mod fixture;
use fixture::{Fixture, seal};

#[test]
fn mixed_core_calls_share_one_arena_and_preserve_selection_and_effect() {
    let fixture = Fixture::new();
    let (module, protocols, dependencies) = fixture.mixed(false);
    assert_eq!(module.meta.external_callables.len(), 2);
    let (ordinary, protocol) = {
        let mut callables = module.meta.external_callables.iter();
        let (ordinary, value) = callables.next().unwrap();
        assert!(matches!(
            value.selection(),
            ExternalCallableSelection::Dependency(_)
        ));
        assert_eq!(value.gc_effect(), GcEffect::NoGc);
        let (protocol, value) = callables.next().unwrap();
        assert!(matches!(
            value.selection(),
            ExternalCallableSelection::InitializationCycle(_)
        ));
        assert_eq!(value.gc_effect(), GcEffect::Managed);
        (ordinary, protocol)
    };
    assert_ne!(ordinary, protocol);

    let output = DependencyMirOutput::try_new(module, protocols, dependencies).unwrap();
    let (module, protocols, dependencies) = output.into_parts();
    let input = seal(module, &protocols, &dependencies).unwrap();
    let roots = input.materialization();
    assert_eq!(roots.imported_core_callable_roots()[0].callable(), protocol);
    assert_eq!(
        roots.imported_dependency_callable_roots()[0].callable(),
        ordinary
    );
    assert_eq!(
        roots.imported_dependency_callable_roots()[0].provider(),
        ConeIdentity::CORE
    );
    assert_eq!(
        roots.imported_dependency_callable_roots()[0].gc_effect(),
        GcEffect::NoGc
    );
}

#[test]
fn output_and_sealer_reject_an_unreferenced_entry_from_either_selection() {
    let fixture = Fixture::new();
    for index in 0..2 {
        for use_output in [true, false] {
            let (mut module, protocols, dependencies) = fixture.mixed(false);
            let function = module.functions.iter_mut().next().unwrap().1;
            function.body.blocks[function.body.entry]
                .statements
                .remove(index);
            let error = if use_output {
                match DependencyMirOutput::try_new(module, protocols, dependencies) {
                    Err(DependencyMirOutputError::ExternalCallables(error)) => error,
                    _ => panic!("the output must reject the unreferenced external use"),
                }
            } else {
                seal(module, &protocols, &dependencies).err().unwrap()
            };
            assert!(matches!(error,
                SingleConeStrongMirInputError::UnreferencedExternalCallable { index: found }
                if found == index as u32));
        }
    }
}

#[test]
fn output_and_sealer_reject_an_implementation_shared_by_both_selections() {
    let fixture = Fixture::new();
    for use_output in [true, false] {
        let (module, protocols, dependencies) = fixture.mixed(true);
        let error = if use_output {
            match DependencyMirOutput::try_new(module, protocols, dependencies) {
                Err(DependencyMirOutputError::ExternalCallables(error)) => error,
                _ => panic!("the output must reject a duplicate external implementation"),
            }
        } else {
            seal(module, &protocols, &dependencies).err().unwrap()
        };
        assert!(matches!(error,
            SingleConeStrongMirInputError::DuplicateExternalImplementation { implementation }
            if implementation == StrongCallableDefinitionOwner::Function(fixture.cycle)));
    }
}
