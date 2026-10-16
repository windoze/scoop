use super::*;
use crate::*;

mod support;
use support::*;

#[test]
fn binds_exact_roles_from_generated_callable_roots() {
    let module = fixture();
    assert!(module.meta.generated_callables.is_empty());
    let identity = module.initialization_units[unit_id()].identity.id();
    let ensure = module.initialization_units[unit_id()].ensure;
    let initializer = module.initialization_units[unit_id()].initializer;
    let input = bind_input(module).unwrap();
    let root = input.materialization().initialization_roots()[0];
    assert_eq!(root.unit(), unit_id());
    assert_eq!(root.identity(), identity);
    assert_eq!(root.ensure().function(), ensure);
    assert_eq!(root.initializer().function(), initializer);
    assert_ne!(root.ensure().subject(), root.initializer().subject());
    assert_eq!(
        input.materialization().initialization_units(),
        &[root.unit()]
    );
}

#[test]
fn each_role_must_be_an_emitted_body_not_only_a_foundation_signature() {
    let mut module = fixture();
    let ensure = module.initialization_units[unit_id()].ensure;
    module.top_level.retain(|function| *function != ensure);
    assert!(
        matches!(bind_input(module), Err(ConeMirInputError::Initialization(
        StrongInitializationUnitError::MissingBody { function, .. }
    )) if function == ensure)
    );
}
