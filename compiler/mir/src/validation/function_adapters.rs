use std::collections::HashSet;

use la_arena::{Arena, Idx};

use super::*;

pub(super) fn validate_function_adapter_metadata(
    module: &Module,
) -> Result<(), MirValidationError> {
    let mut classes = HashSet::new();
    let mut static_shapes = HashSet::new();
    for (adapter_id, adapter) in module.meta.closure_adapters.iter() {
        let location = MirValidationLocation::FunctionAdapter {
            adapter: adapter_id,
        };
        if !static_shapes.insert((adapter.source(), adapter.target())) {
            return invalid(
                location,
                "the same static adapter shape is recorded more than once",
            );
        }
        if !classes.insert(adapter.class()) {
            return invalid(location, "an adapter class is claimed more than once");
        }
        let Some(source) = arena_get(&module.function_types, adapter.source()) else {
            return invalid(location, "the source function type does not exist");
        };
        let Some(target) = arena_get(&module.function_types, adapter.target()) else {
            return invalid(location, "the target function type does not exist");
        };
        validate_class(
            module,
            location,
            adapter.class(),
            adapter.target(),
            &Type::Function(adapter.source()),
            target,
        )?;
        if source.is_suspend != target.is_suspend
            || source.parameter_types.len() != target.parameter_types.len()
        {
            return invalid(
                location,
                "static function variance must preserve effect and arity",
            );
        }
    }

    let mut dynamic_targets = HashSet::new();
    for (adapter_id, adapter) in module.meta.dynamic_closure_adapters.iter() {
        let location = MirValidationLocation::DynamicFunctionAdapter {
            adapter: adapter_id,
        };
        if !dynamic_targets.insert(adapter.target()) {
            return invalid(
                location,
                "the same dynamic adapter target is recorded more than once",
            );
        }
        if !classes.insert(adapter.class()) {
            return invalid(location, "an adapter class is claimed more than once");
        }
        let Some(target) = arena_get(&module.function_types, adapter.target()) else {
            return invalid(location, "the target function type does not exist");
        };
        validate_class(
            module,
            location,
            adapter.class(),
            adapter.target(),
            &Type::Any,
            target,
        )?;
    }
    Ok(())
}

fn validate_class(
    module: &Module,
    location: MirValidationLocation,
    class_id: ClosureClassId,
    target_id: FunctionTypeId,
    capture_type: &Type,
    target: &FunctionType,
) -> Result<(), MirValidationError> {
    let Some(class) = arena_get(&module.closure_classes, class_id) else {
        return invalid(location, "the adapter closure class does not exist");
    };
    if class.function_type != target_id {
        return invalid(
            location,
            "the adapter class exposes a different function type",
        );
    }
    if class.captures.len() != 1 || class.captures[0].ty != *capture_type {
        return invalid(location, "the adapter class has an invalid source capture");
    }
    let Some(invoke) = arena_get(&module.closure_invoke_functions, class.invoke) else {
        return invalid(location, "the adapter invoke relation does not exist");
    };
    let Some(function) = arena_get(&module.functions, invoke.function) else {
        return invalid(location, "the adapter invoke function does not exist");
    };
    let hidden_parameter_count = if target.is_suspend {
        let Some(coroutine) = module
            .meta
            .coroutine_functions
            .iter()
            .map(|(_, coroutine)| coroutine)
            .find(|coroutine| coroutine.function == invoke.function)
        else {
            return invalid(
                location,
                "the suspend adapter invoke has no coroutine metadata",
            );
        };
        let Some(step) = arena_get(&module.meta.coroutine_steps, coroutine.step) else {
            return invalid(location, "the suspend adapter has no coroutine step");
        };
        if coroutine.source_return != target.return_type
            || function.return_ty != Type::Enum(step.enum_id(), Vec::new())
        {
            return invalid(
                location,
                "the suspend adapter invoke result does not match its target",
            );
        }
        2
    } else {
        if function.return_ty != target.return_type {
            return invalid(
                location,
                "the adapter invoke result does not match its target",
            );
        }
        1
    };
    if function.params.len().checked_sub(hidden_parameter_count)
        != Some(target.parameter_types.len())
        || function.params[0].ty != Type::Function(target_id)
        || function.params[1..=target.parameter_types.len()]
            .iter()
            .map(|parameter| &parameter.ty)
            .ne(target.parameter_types.iter())
    {
        return invalid(
            location,
            "the adapter invoke parameters do not match its target",
        );
    }
    Ok(())
}

fn invalid<T>(
    location: MirValidationLocation,
    reason: &'static str,
) -> Result<T, MirValidationError> {
    Err(MirValidationError {
        location,
        kind: MirValidationErrorKind::InvalidFunctionAdapter { reason },
    })
}

fn arena_get<T>(arena: &Arena<T>, id: Idx<T>) -> Option<&T> {
    ((id.into_raw().into_u32() as usize) < arena.len()).then(|| &arena[id])
}
