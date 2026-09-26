use super::*;
use scoop_slib::CrossConeProtocolImportError;

pub(super) fn check(
    closure: &scoop_slib::ValidatedCrossConeSemanticClosure,
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
    assert!(matches!(
        closure.import_compiler_protocols(ordinary.identity()),
        Err(CrossConeProtocolImportError::MissingDefinitions(provider)) if provider == ordinary.identity()
    ));
    assert!(matches!(
        closure.import_compiler_protocols(closure.current()),
        Err(CrossConeProtocolImportError::MissingProvider(provider)) if provider == closure.current()
    ));
}
