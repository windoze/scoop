use super::*;
use scoop_identity::{Effect, ExactCallableSignature};

pub(super) fn validate(
    module: &Module,
    coroutine: &CoroutineFunction,
    step: &CoroutineStep,
    function: &Function,
    location: MirValidationLocation,
) -> Result<(), MirValidationError> {
    let invalid_completion = || {
        error(
            location,
            "a suspend callable must end in its exact continuation parameter",
        )
    };
    let completion = function.params.last().ok_or_else(invalid_completion)?;
    let Type::Interface(interface) = &completion.ty else {
        return Err(invalid_completion());
    };
    if arena_get(&module.interfaces, *interface).is_none_or(|interface| {
        interface.type_arguments.as_slice() != std::slice::from_ref(&coroutine.source_return)
    }) {
        return Err(invalid_completion());
    }
    let continuation = source_exact_type(module, &completion.ty).ok_or_else(invalid_completion)?;
    let exact_step = module
        .meta
        .generated_exact_types
        .get(GeneratedExactTypeLocation::Enum(step.enum_id()))
        .ok_or_else(|| error(location, "the coroutine step has no exact type relation"))?
        .exact_record()
        .id();
    let source = &coroutine.logical_signature;
    let mut parameters = source.parameters().to_vec();
    parameters.push(continuation);
    let expected = ExactCallableSignature::new(
        Effect::Ordinary,
        source.receiver().into_option(),
        parameters,
        exact_step,
    );
    if coroutine.lowered_signature != expected {
        return Err(error(
            location,
            "coroutine lowered signature does not match its continuation and step ABI",
        ));
    }
    Ok(())
}
