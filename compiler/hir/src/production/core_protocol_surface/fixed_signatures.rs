use scoop_identity::{Effect, NonEmptyVec, SignatureCallableShape, SignatureTypeKey};

use super::*;

pub(super) fn validate_fixed_callable_signatures(
    surface: &CoreCompilerProtocolSurfaceV1,
) -> Result<(), CoreCompilerProtocolSurfaceRelationError> {
    let fundamental = &surface.fundamental_types;
    let unit = SignatureTypeKey::Nominal(fundamental.unit);
    let string = SignatureTypeKey::Nominal(fundamental.string);
    if !matches!(
        surface.program_arguments.definition(),
        crate::CoreProtocolCallableDefinitionV1::Function(_)
    ) || surface.program_arguments.signature()
        != &signature(
            Effect::Ordinary,
            Vec::new(),
            application(fundamental.array, string.clone()),
        )
    {
        return Err(CoreCompilerProtocolSurfaceRelationError::ProgramArgumentsSignature);
    }
    let binder = SignatureTypeKey::Binder { depth: 0, index: 0 };

    validate(
        &surface.iteration_protocol.next,
        CoreProtocolProductKindV1::Iteration,
        1,
        signature(
            Effect::Ordinary,
            Vec::new(),
            application(surface.option_protocol.option, binder.clone()),
        ),
    )?;

    let coroutine = &surface.coroutine_protocol;
    validate(
        &coroutine.continuation_resume,
        CoreProtocolProductKindV1::Coroutine,
        1,
        signature(Effect::Ordinary, vec![binder.clone()], unit.clone()),
    )?;
    validate(
        &coroutine.continuation_resume_with_exception,
        CoreProtocolProductKindV1::Coroutine,
        3,
        signature(
            Effect::Ordinary,
            vec![SignatureTypeKey::Nominal(
                surface.exception_protocol.throwable,
            )],
            unit.clone(),
        ),
    )?;
    validate(
        &coroutine.suspend_task_run,
        CoreProtocolProductKindV1::Coroutine,
        6,
        signature(Effect::Suspend, Vec::new(), binder.clone()),
    )?;
    validate(
        &coroutine.suspend_registration_register,
        CoreProtocolProductKindV1::Coroutine,
        9,
        signature(
            Effect::Ordinary,
            vec![application(coroutine.continuation, binder)],
            unit,
        ),
    )?;

    let exceptions = &surface.exception_protocol;
    for (index, callable, owner) in [
        (1, &exceptions.throwable_constructor, exceptions.throwable),
        (
            3,
            &exceptions.unwrap_exception_constructor,
            exceptions.unwrap_exception,
        ),
        (
            5,
            &exceptions.class_cast_exception_constructor,
            exceptions.class_cast_exception,
        ),
        (
            7,
            &exceptions.arithmetic_exception_constructor,
            exceptions.arithmetic_exception,
        ),
        (
            9,
            &exceptions.index_out_of_bounds_exception_constructor,
            exceptions.index_out_of_bounds_exception,
        ),
        (
            11,
            &exceptions.illegal_state_exception_constructor,
            exceptions.illegal_state_exception,
        ),
    ] {
        validate(
            callable,
            CoreProtocolProductKindV1::Exception,
            index,
            signature(
                Effect::Ordinary,
                Vec::new(),
                SignatureTypeKey::Nominal(owner),
            ),
        )?;
    }
    validate(
        &exceptions.initialization_cycle_thrower,
        CoreProtocolProductKindV1::Exception,
        12,
        signature(
            Effect::Ordinary,
            vec![string.clone()],
            SignatureTypeKey::Nominal(fundamental.unit),
        ),
    )?;
    validate(
        &exceptions.missing_context_exception_constructor,
        CoreProtocolProductKindV1::Exception,
        16,
        signature(
            Effect::Ordinary,
            vec![application(surface.option_protocol.option, string)],
            SignatureTypeKey::Nominal(exceptions.missing_context_exception),
        ),
    )
}

fn validate(
    callable: &CoreProtocolCallableV1,
    product: CoreProtocolProductKindV1,
    index: usize,
    expected: SignatureCallableShape,
) -> Result<(), CoreCompilerProtocolSurfaceRelationError> {
    if callable.signature() == &expected {
        Ok(())
    } else {
        Err(
            CoreCompilerProtocolSurfaceRelationError::FixedCallableSignatureMismatch {
                product,
                index,
            },
        )
    }
}

fn signature(
    effect: Effect,
    parameters: Vec<SignatureTypeKey>,
    result: SignatureTypeKey,
) -> SignatureCallableShape {
    SignatureCallableShape::new(effect, None, parameters, result)
}

fn application(origin: PersistentGenericTypeId, argument: SignatureTypeKey) -> SignatureTypeKey {
    SignatureTypeKey::NominalApplication {
        origin,
        arguments: NonEmptyVec::from_first(argument, []),
    }
}
