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

    for (step_id, step) in module.meta.coroutine_steps.iter() {
        if CoroutineStep::checked(
            &module.enums,
            step.completed_payload(),
            step.suspended(),
            step.result().clone(),
        )
        .is_none()
        {
            return Err(MirValidationError {
                location: MirValidationLocation::CoroutineStep { step: step_id },
                kind: MirValidationErrorKind::InvalidCoroutineStep,
            });
        }
    }

    for (slot_id, slot) in module.meta.coroutine_slots.iter() {
        if CoroutineSlot::checked(
            &module.enums,
            slot.value_payload(),
            slot.empty(),
            slot.value().clone(),
        )
        .is_none()
        {
            return Err(MirValidationError {
                location: MirValidationLocation::CoroutineSlot { slot: slot_id },
                kind: MirValidationErrorKind::InvalidCoroutineSlot,
            });
        }
    }

    Ok(())
}
