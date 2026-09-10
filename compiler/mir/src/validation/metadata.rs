use std::collections::HashSet;

use super::*;

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

    let mut exact_types = Vec::new();
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
        if !step_results.insert(exact)
            || !register_exact_type(&mut exact_types, step.result(), exact)
        {
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
        if !slot_values.insert(exact) || !register_exact_type(&mut exact_types, slot.value(), exact)
        {
            return Err(MirValidationError {
                location: MirValidationLocation::CoroutineSlot { slot: slot_id },
                kind: MirValidationErrorKind::InvalidCoroutineSlot,
            });
        }
    }

    Ok(())
}

fn register_exact_type(
    registered: &mut Vec<(Type, scoop_identity::PersistentExactTypeId)>,
    ty: &Type,
    exact: scoop_identity::PersistentExactTypeId,
) -> bool {
    if let Some((found_ty, found_exact)) = registered
        .iter()
        .find(|(found_ty, found_exact)| found_ty == ty || *found_exact == exact)
    {
        found_ty == ty && *found_exact == exact
    } else {
        registered.push((ty.clone(), exact));
        true
    }
}
