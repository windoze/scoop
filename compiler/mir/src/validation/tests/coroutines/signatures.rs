use super::*;
use scoop_identity::{Effect, ExactCallableSignature};

#[test]
fn coroutine_physical_signature_must_match_its_actual_continuation_and_step() {
    for changed in 0..4 {
        let mut fixture = coroutine_fixture(false);
        let (_, coroutine) = fixture
            .module
            .meta
            .coroutine_functions
            .iter_mut()
            .next()
            .unwrap();
        let signature = &coroutine.lowered_signature;
        let unit = test_exact_type(&Type::Unit).id();
        coroutine.lowered_signature = ExactCallableSignature::new(
            if changed == 0 {
                Effect::Suspend
            } else {
                signature.effect()
            },
            if changed == 1 {
                Some(unit)
            } else {
                signature.receiver().into_option()
            },
            if changed == 2 {
                vec![unit]
            } else {
                signature.parameters().to_vec()
            },
            if changed == 3 {
                unit
            } else {
                signature.result()
            },
        );
        install_callable_signatures(&mut fixture.module);
        assert!(
            matches!(
                fixture.module.validate(),
                Err(MirValidationError {
                    kind: MirValidationErrorKind::InvalidCoroutineMetadata {
                        reason: "coroutine lowered signature does not match its continuation and step ABI"
                    },
                    ..
                })
            ),
            "changed signature component {changed}"
        );
    }
}

#[test]
fn coroutine_completion_parameter_cannot_be_a_scalar() {
    let mut fixture = coroutine_fixture(false);
    let function = fixture
        .module
        .meta
        .coroutine_functions
        .iter()
        .next()
        .unwrap()
        .1
        .function;
    let function = &mut fixture.module.functions[function];
    let completion = function.params.last_mut().unwrap();
    completion.ty = Type::Boolean;
    function.body.locals[completion.local].ty = Type::Boolean;
    assert!(matches!(
        fixture.module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::InvalidCoroutineMetadata {
                reason: "a suspend callable must end in its exact continuation parameter"
            },
            ..
        })
    ));
}
