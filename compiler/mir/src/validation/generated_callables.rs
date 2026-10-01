use std::collections::HashSet;

use la_arena::{Arena, Idx};

use super::*;

pub(super) fn validate_generated_callable_metadata(
    module: &Module,
) -> Result<(), MirValidationError> {
    for (index, entry) in module.meta.generated_callables.iter().enumerate() {
        let location = MirValidationLocation::GeneratedCallable {
            entry: index as u32,
        };
        if arena_get(&module.functions, entry.function()).is_none() {
            return invalid(location, "the generated callable function does not exist");
        }
        if module
            .meta
            .source_callable_materializations
            .get(entry.function())
            .is_some()
        {
            return invalid(
                location,
                "the function is also claimed by a source callable materialization",
            );
        }
    }

    let mut expected = HashSet::new();
    for (_, bridge) in module.callback_bridges.iter() {
        let Some((_, bridge_function)) = bridge.local_definition() else {
            continue;
        };
        expect(
            module,
            &mut expected,
            bridge_function,
            bridge.identity().callable_record(),
            bridge.identity().signature_record().subject(),
        )?;
    }
    for (_, adapter) in module.foreign_callback_adapters.iter() {
        expect(
            module,
            &mut expected,
            adapter.function,
            adapter.identity_record(),
            adapter.signature_subject(),
        )?;
    }
    for (_, adapter) in module.meta.closure_adapters.iter() {
        expect_closure_adapter(
            module,
            &mut expected,
            adapter.class(),
            adapter.identity().callable_record(),
            adapter.identity().callable_signature_record().subject(),
        )?;
    }
    for (_, adapter) in module.meta.dynamic_closure_adapters.iter() {
        expect_closure_adapter(
            module,
            &mut expected,
            adapter.class(),
            adapter.identity().callable_record(),
            adapter.identity().callable_signature_record().subject(),
        )?;
    }
    for bridge in &module.meta.function_bridges {
        expect(
            module,
            &mut expected,
            bridge.function(),
            bridge.identity().callable_record(),
            bridge.identity().signature_record().subject(),
        )?;
    }
    for (_, coroutine) in module.meta.coroutine_functions.iter() {
        if let CoroutineLowering::StateMachine {
            driver,
            driver_identity,
            ..
        } = &coroutine.lowering
        {
            expect(
                module,
                &mut expected,
                *driver,
                driver_identity.callable_record(),
                driver_identity.signature_record().subject(),
            )?;
        }
    }
    for start in &module.meta.coroutine_starts {
        expect(
            module,
            &mut expected,
            start.function(),
            start.identity().callable_record(),
            start.identity().signature_record().subject(),
        )?;
    }
    for (_, point) in module.meta.coroutine_resume_points.iter() {
        expect(
            module,
            &mut expected,
            point.resume(),
            point.identity().success().callable_record(),
            point.identity().success().signature_record().subject(),
        )?;
        expect(
            module,
            &mut expected,
            point.resume_with_exception(),
            point.identity().failure().callable_record(),
            point.identity().failure().signature_record().subject(),
        )?;
    }
    for adjust in &module.meta.boxing_adjusts {
        expect(
            module,
            &mut expected,
            adjust.function(),
            adjust.identity().callable_record(),
            adjust.identity().signature_record().subject(),
        )?;
    }
    if expected.len() != module.meta.generated_callables.len() {
        return invalid(
            next_location(module),
            "the relation contains an unclaimed MIR-generated callable",
        );
    }
    Ok(())
}

fn expect_closure_adapter(
    module: &Module,
    expected: &mut HashSet<FunctionId>,
    class: ClosureClassId,
    identity: &GeneratedCallableRecord,
    signature_subject: CallableSignatureSubject,
) -> Result<(), MirValidationError> {
    let function = arena_get(&module.closure_classes, class)
        .and_then(|class| arena_get(&module.closure_invoke_functions, class.invoke))
        .map(|invoke| invoke.function)
        .ok_or_else(|| {
            error(
                next_location(module),
                "a generated closure adapter has no invoke function",
            )
        })?;
    expect(module, expected, function, identity, signature_subject)
}

fn expect(
    module: &Module,
    expected: &mut HashSet<FunctionId>,
    function: FunctionId,
    identity: &GeneratedCallableRecord,
    signature_subject: CallableSignatureSubject,
) -> Result<(), MirValidationError> {
    let location = next_location(module);
    if !expected.insert(function) {
        return invalid(
            location,
            "one function is claimed by multiple MIR-generated callables",
        );
    }
    let Some(entry) = module.meta.generated_callables.get(function) else {
        return invalid(
            location,
            "a MIR-generated callable has no typed function materialization",
        );
    };
    if entry.identity_record() != identity {
        return invalid(
            location,
            "the function materialization names a different generated callable",
        );
    }
    if entry.signature_subject() != signature_subject {
        return invalid(
            location,
            "the function materialization names a different signature subject",
        );
    }
    Ok(())
}

fn next_location(module: &Module) -> MirValidationLocation {
    MirValidationLocation::GeneratedCallable {
        entry: module.meta.generated_callables.len() as u32,
    }
}

fn invalid(
    location: MirValidationLocation,
    reason: &'static str,
) -> Result<(), MirValidationError> {
    Err(error(location, reason))
}

fn error(location: MirValidationLocation, reason: &'static str) -> MirValidationError {
    MirValidationError {
        location,
        kind: MirValidationErrorKind::InvalidGeneratedCallable { reason },
    }
}

fn arena_get<T>(arena: &Arena<T>, id: Idx<T>) -> Option<&T> {
    arena
        .iter()
        .find_map(|(candidate, value)| (candidate == id).then_some(value))
}
