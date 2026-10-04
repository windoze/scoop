use scoop_identity::{Effect, NonEmptyVec, SignatureCallableShape, SignatureTypeKey};

use super::*;

pub(super) fn validate_fixed_callable_signatures(
    surface: &CoreCompilerProtocolSurfaceV1,
) -> Result<(), CoreCompilerProtocolSurfaceRelationError> {
    let fundamental = surface.fundamental_types.entries();
    let unit = concrete_type(fundamental, 0);
    let string = concrete_type(fundamental, 10);
    let binder = SignatureTypeKey::Binder { depth: 0, index: 0 };

    validate(
        surface.iteration_protocol.entries(),
        CoreProtocolProductKindV1::Iteration,
        1,
        signature(
            Effect::Ordinary,
            Vec::new(),
            application(surface.option_protocol.entries(), 0, binder.clone()),
        ),
    )?;

    let coroutine = surface.coroutine_protocol.entries();
    validate(
        coroutine,
        CoreProtocolProductKindV1::Coroutine,
        1,
        signature(Effect::Ordinary, vec![binder.clone()], unit.clone()),
    )?;
    validate(
        coroutine,
        CoreProtocolProductKindV1::Coroutine,
        3,
        signature(
            Effect::Ordinary,
            vec![concrete_type(surface.exception_protocol.entries(), 0)],
            unit.clone(),
        ),
    )?;
    validate(
        coroutine,
        CoreProtocolProductKindV1::Coroutine,
        6,
        signature(Effect::Suspend, Vec::new(), binder.clone()),
    )?;
    validate(
        coroutine,
        CoreProtocolProductKindV1::Coroutine,
        9,
        signature(
            Effect::Ordinary,
            vec![application(coroutine, 0, binder)],
            unit,
        ),
    )?;

    let exceptions = surface.exception_protocol.entries();
    for (callable, owner) in [(1, 0), (3, 2), (5, 4), (7, 6), (9, 8), (11, 10)] {
        validate(
            exceptions,
            CoreProtocolProductKindV1::Exception,
            callable,
            signature(
                Effect::Ordinary,
                Vec::new(),
                concrete_type(exceptions, owner),
            ),
        )?;
    }
    validate(
        exceptions,
        CoreProtocolProductKindV1::Exception,
        12,
        signature(
            Effect::Ordinary,
            vec![string.clone()],
            concrete_type(fundamental, 0),
        ),
    )?;
    validate(
        exceptions,
        CoreProtocolProductKindV1::Exception,
        16,
        signature(
            Effect::Ordinary,
            vec![application(surface.option_protocol.entries(), 0, string)],
            concrete_type(exceptions, 15),
        ),
    )
}

fn validate<const N: usize>(
    entries: &[CoreProtocolEntryV1; N],
    product: CoreProtocolProductKindV1,
    index: usize,
    expected: SignatureCallableShape,
) -> Result<(), CoreCompilerProtocolSurfaceRelationError> {
    if callable_entry_ref(entries, index).signature() == &expected {
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

fn concrete_type<const N: usize>(
    entries: &[CoreProtocolEntryV1; N],
    index: usize,
) -> SignatureTypeKey {
    SignatureTypeKey::Nominal(concrete_entry(entries, index))
}

fn application<const N: usize>(
    entries: &[CoreProtocolEntryV1; N],
    index: usize,
    argument: SignatureTypeKey,
) -> SignatureTypeKey {
    let origin = match entries[index] {
        CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(id)) => id,
        _ => unreachable!("validated core protocol role has its fixed subject kind"),
    };
    SignatureTypeKey::NominalApplication {
        origin,
        arguments: NonEmptyVec::from_first(argument, []),
    }
}
