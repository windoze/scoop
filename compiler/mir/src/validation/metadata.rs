use std::collections::HashSet;

use super::*;

pub(super) fn validate_source_callable_materializations(
    module: &Module,
) -> Result<(), MirValidationError> {
    for entry in module.meta.source_callable_materializations.iter() {
        if arena_get(&module.functions, entry.function()).is_none() {
            return Err(MirValidationError {
                location: MirValidationLocation::SourceCallableMaterialization {
                    function: entry.function(),
                },
                kind: MirValidationErrorKind::InvalidSourceCallableMaterialization {
                    reason: "the function does not exist",
                },
            });
        }
        let signature = entry.signature_record().signature();
        let receiver = match signature.receiver() {
            scoop_identity::OptionalExactOwner::Absent => None,
            scoop_identity::OptionalExactOwner::Present(receiver) => Some(receiver),
        };
        if receiver
            .into_iter()
            .chain(signature.parameters().iter().copied())
            .chain(std::iter::once(signature.result()))
            .any(|exact| {
                module
                    .meta
                    .source_exact_types
                    .get_by_identity(exact)
                    .is_none()
            })
        {
            return Err(MirValidationError {
                location: MirValidationLocation::SourceCallableMaterialization {
                    function: entry.function(),
                },
                kind: MirValidationErrorKind::InvalidSourceCallableMaterialization {
                    reason: "the logical signature references an unknown source exact type",
                },
            });
        }
    }
    Ok(())
}

pub(super) fn validate_local_value_metadata(module: &Module) -> Result<(), MirValidationError> {
    for entry in module.meta.local_values.iter() {
        let location = MirValidationLocation::LocalValue {
            owner: entry.owner(),
            local: entry.local(),
        };
        let function = match entry.owner() {
            LocalValueOwner::Function(function) => arena_get(&module.functions, function),
            LocalValueOwner::ReleaseHook(hook) => {
                arena_get(&module.release_hooks, hook).map(|hook| &hook.code)
            }
        };
        let Some(function) = function else {
            return Err(MirValidationError {
                location,
                kind: MirValidationErrorKind::InvalidLocalValue {
                    reason: "the owning function does not exist",
                },
            });
        };
        if arena_get(&function.body.locals, entry.local()).is_none() {
            return Err(MirValidationError {
                location,
                kind: MirValidationErrorKind::InvalidLocalValue {
                    reason: "the local does not exist in the owning function body",
                },
            });
        }
    }
    Ok(())
}

pub(super) fn validate_enum_metadata(module: &Module) -> Result<(), MirValidationError> {
    for option in module.option_core.iter().copied() {
        if OptionCore::checked(&module.enums, option.some_payload(), option.none()) != Some(option)
        {
            return Err(MirValidationError {
                location: MirValidationLocation::OptionCore {
                    enumeration: option.enum_id(),
                },
                kind: MirValidationErrorKind::InvalidOptionCore,
            });
        }
    }

    let mut step_results = HashSet::new();
    for (step_id, step) in module.meta.coroutine_steps.iter() {
        if CoroutineStep::checked(
            &module.enums,
            step.completed_payload(),
            step.suspended(),
            step.result().clone(),
            step.identity().clone(),
        )
        .is_none()
        {
            return Err(MirValidationError {
                location: MirValidationLocation::CoroutineStep { step: step_id },
                kind: MirValidationErrorKind::InvalidCoroutineStep,
            });
        }
        let exact = step.identity().result_record().id();
        if !step_results.insert(exact) || source_exact_type(module, step.result()) != Some(exact) {
            return Err(MirValidationError {
                location: MirValidationLocation::CoroutineStep { step: step_id },
                kind: MirValidationErrorKind::InvalidCoroutineStep,
            });
        }
    }

    let mut slot_values = HashSet::new();
    for (slot_id, slot) in module.meta.coroutine_slots.iter() {
        if CoroutineSlot::checked(
            &module.enums,
            slot.value_payload(),
            slot.empty(),
            slot.value().clone(),
            slot.identity().clone(),
        )
        .is_none()
        {
            return Err(MirValidationError {
                location: MirValidationLocation::CoroutineSlot { slot: slot_id },
                kind: MirValidationErrorKind::InvalidCoroutineSlot,
            });
        }
        let exact = slot.identity().value_record().id();
        if !slot_values.insert(exact) || source_exact_type(module, slot.value()) != Some(exact) {
            return Err(MirValidationError {
                location: MirValidationLocation::CoroutineSlot { slot: slot_id },
                kind: MirValidationErrorKind::InvalidCoroutineSlot,
            });
        }
    }

    Ok(())
}

pub(super) fn source_exact_type(
    module: &Module,
    ty: &Type,
) -> Option<scoop_identity::PersistentExactTypeId> {
    module
        .meta
        .source_exact_types
        .get(ty)
        .map(|source| source.identity_record().id())
}

fn arena_get<T>(arena: &Arena<T>, id: Idx<T>) -> Option<&T> {
    ((id.into_raw().into_u32() as usize) < arena.len()).then(|| &arena[id])
}
