use super::*;
use scoop_mir::SelectedExternalMirSet as Selection;
use scoop_slib::{CrossConeInitializationSelectionError as Error, CrossConeProtocolImportError};

pub(super) fn check(
    closure: &scoop_slib::ValidatedCrossConeSemanticClosure<'_>,
    core: &Compile<'_, '_>,
    ordinary: &Compile<'_, '_>,
) {
    let inputs = closure.import_compiler_protocols(core.identity()).unwrap();
    let cycle = inputs
        .protocols()
        .exceptions()
        .initialization_cycle_thrower();
    let string = inputs.protocols().fundamental_types().string();
    assert_eq!(cycle.provider(), core.identity());
    assert_eq!(string.provider(), core.identity());
    assert_eq!(
        core.hir().identity(string.persistent()),
        Some(string.identity())
    );
    let selected = closure
        .select_initialization_cycle(Selection::empty(closure.current()), cycle)
        .unwrap();
    assert_eq!(selected.len(), 1);
    assert!(matches!(
        closure.select_initialization_cycle(selected, cycle),
        Err(Error::Selection(
            scoop_mir::SelectedExternalMirSetBuildError::DuplicateInitializationCycle
        ))
    ));
    assert!(matches!(
        closure.select_initialization_cycle(Selection::empty(core.identity()), cycle),
        Err(Error::ConsumerMismatch { .. })
    ));
    for callable in [
        inputs.protocols().exceptions().throwable_constructor(),
        inputs.protocols().ffi().gc_unpin_raw(),
    ] {
        let error = closure
            .select_initialization_cycle(Selection::empty(closure.current()), callable)
            .err()
            .expect("constructors and generic functions cannot name the service");
        assert!(matches!(error, Error::InvalidDefinition(_)), "{error:?}");
    }
    let error = closure
        .select_initialization_cycle(
            Selection::empty(closure.current()),
            inputs.protocols().source_location().current(),
        )
        .err()
        .expect("another source function cannot replace the service");
    assert!(matches!(error, Error::SourceMismatch { .. }), "{error:?}");
    assert!(matches!(
        closure.import_compiler_protocols(ordinary.identity()),
        Err(CrossConeProtocolImportError::MissingDefinitions(provider)) if provider == ordinary.identity()
    ));
    assert!(matches!(
        closure.import_compiler_protocols(closure.current()),
        Err(CrossConeProtocolImportError::MissingProvider(provider)) if provider == closure.current()
    ));
}
