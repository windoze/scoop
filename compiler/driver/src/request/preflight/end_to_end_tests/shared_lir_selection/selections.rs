use super::*;
use scoop_identity::{
    CallableOwner, CallableRole, DependencyCallableDeclarationId, Effect, ExactCallableSignature,
    StrongCallableDefinitionOwner,
};
use scoop_mir::{SelectedDependencyMirCallableV1 as Record, SelectedExternalMirSet as Selection};
use scoop_slib::CrossConeLirSelectionProjectionError as Error;

pub(super) fn check(
    closure: &scoop_slib::ValidatedCrossConeSemanticClosure,
    core: &Compile<'_, '_>,
    ordinary: &Compile<'_, '_>,
) {
    let source = core
        .production()
        .mir_core()
        .strong_callable_bridges()
        .initialization_cycle()
        .unwrap();
    let CallableOwner::Function(function) = source.implementation() else {
        panic!("initialization service is a source function")
    };
    let service = Record::try_new(
        core.identity(),
        DependencyCallableDeclarationId::Function(function),
        StrongCallableDefinitionOwner::Function(function),
        source.signature().clone(),
    )
    .unwrap();
    let exports = ordinary.production().mir_cross_cone().exports();
    assert_eq!(exports.len(), 1);
    let export = &exports[0];
    let ordinary = Record::try_new(
        ordinary.identity(),
        export.declaration(),
        export.implementation(),
        export.signature().clone(),
    )
    .unwrap();
    let inputs = closure.import_compiler_protocols(core.identity()).unwrap();
    for records in [vec![], vec![ordinary.clone()]] {
        let mir = closure
            .select_initialization_cycle(
                Selection::try_from_callables(closure.current(), records).unwrap(),
                inputs
                    .protocols()
                    .exceptions()
                    .initialization_cycle_thrower(),
            )
            .unwrap();
        let lir = closure.project_dependency_callables_to_lir(&mir).unwrap();
        assert_eq!(lir.len(), mir.len());
        for selected in mir.callables() {
            let actual = lir
                .callable(
                    lir.callable_for(selected.provider(), selected.declaration())
                        .unwrap(),
                )
                .unwrap();
            assert_eq!(actual.role(), selected.role());
            assert_eq!(actual.bridge().target(), selected.implementation());
            assert_eq!(
                actual.bridge().abi_signature().signature(),
                selected.signature()
            );
        }
        assert_eq!(
            lir.callable(lir.initialization_cycle().unwrap())
                .unwrap()
                .role(),
            CallableRole::InitializationCycle
        );
    }
    let ordinary_service =
        Selection::try_from_callables(closure.current(), vec![service.clone()]).unwrap();
    let lir = closure
        .project_dependency_callables_to_lir(&ordinary_service)
        .unwrap();
    assert_eq!(lir.dependency_callables().count(), 1);
    assert_eq!(
        lir.dependency_callables().next().unwrap().bridge().target(),
        service.implementation()
    );
    let bad_signature =
        ExactCallableSignature::new(Effect::Ordinary, None, vec![], service.signature().result());
    let bad = Record::try_new(
        service.provider(),
        service.declaration(),
        service.implementation(),
        bad_signature,
    )
    .unwrap();
    let wrong_signature = Selection::empty(closure.current())
        .with_initialization_cycle(bad)
        .unwrap();
    assert!(matches!(
        closure.project_dependency_callables_to_lir(&wrong_signature),
        Err(Error::BridgeMismatch { .. })
    ));
    let bad = Record::try_new(
        ConeIdentity::SINGLE_FILE,
        service.declaration(),
        service.implementation(),
        service.signature().clone(),
    )
    .unwrap();
    let missing_provider = Selection::empty(closure.current())
        .with_initialization_cycle(bad)
        .unwrap();
    assert!(matches!(
        closure.project_dependency_callables_to_lir(&missing_provider),
        Err(Error::MissingProvider { .. })
    ));
}
