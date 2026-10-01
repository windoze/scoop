use super::*;

pub(super) fn validate_saved_values(module: &Module) -> Result<(), MirValidationError> {
    for (value_id, value) in module.meta.coroutine_saved_values.iter() {
        let Some(checked) = CoroutineSavedValue::checked(
            &module.classes,
            &module.meta.coroutine_slots,
            value.field(),
            value.slot(),
        ) else {
            return Err(error(
                MirValidationLocation::CoroutineSavedValue { value: value_id },
                "saved field is not the exact CoroutineSlot<T> named by its metadata",
            ));
        };
        if checked.value() != value.value() {
            return Err(error(
                MirValidationLocation::CoroutineSavedValue { value: value_id },
                "saved value type no longer matches its CoroutineSlot payload",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_failure_values(module: &Module) -> Result<(), MirValidationError> {
    for (value_id, value) in module.meta.coroutine_failure_values.iter() {
        if CoroutineFailureValue::checked(
            &module.classes,
            &module.meta.coroutine_slots,
            value.field(),
            value.slot(),
            value.throwable(),
        )
        .is_none()
        {
            return Err(error(
                MirValidationLocation::CoroutineFailureValue { value: value_id },
                "failure field is not the exact CoroutineSlot<Throwable> named by its metadata",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_frame(
    module: &Module,
    frame_id: CoroutineFrameId,
    frame: &CoroutineFrame,
) -> Result<(), MirValidationError> {
    let location = MirValidationLocation::CoroutineFrame { frame: frame_id };
    let Some(coroutine) = arena_get(&module.meta.coroutine_functions, frame.owner()) else {
        return Err(error(
            location,
            "frame refers to an unknown coroutine owner",
        ));
    };
    let rebuilt_identity = CoroutineFrameIdentity::new(
        coroutine.source,
        frame
            .identity()
            .saved_fields()
            .iter()
            .map(|field| field.value_record().clone())
            .collect(),
        coroutine.source_odr_group,
    )
    .map_err(|_| {
        error(
            location,
            "coroutine source cannot form its canonical frame identity",
        )
    })?;
    if &rebuilt_identity != frame.identity() {
        return Err(error(
            location,
            "coroutine frame identity does not match its exact source and saved values",
        ));
    }
    let CoroutineLowering::StateMachine {
        frame: owner_frame,
        driver,
        ..
    } = &coroutine.lowering
    else {
        return Err(error(
            location,
            "a coroutine frame owner must have state-machine lowering",
        ));
    };
    if *owner_frame != frame_id {
        return Err(error(
            location,
            "coroutine frame owner refers to a different frame",
        ));
    }
    for saved in frame.identity().saved_fields() {
        let matching = module
            .meta
            .local_values
            .iter()
            .filter(|value| {
                value.function() == *driver
                    && value.identity_record().id() == saved.value_record().id()
            })
            .count();
        if matching != 1 {
            return Err(error(
                location,
                "each saved field identity must name exactly one local in its coroutine driver",
            ));
        }
    }
    if CoroutineFrame::checked(
        &module.classes,
        &module.meta.coroutine_saved_values,
        &module.meta.coroutine_failure_values,
        frame.class(),
        frame.owner(),
        frame.state(),
        frame.completion(),
        frame.saved_values().to_vec(),
        frame.failure(),
        frame.identity().clone(),
    )
    .is_none()
    {
        return Err(error(
            location,
            "frame roles no longer name distinct fields of the exact frame class",
        ));
    }
    if frame.state().field_index() != 0 || frame.completion().field_index() != 1 {
        return Err(error(
            location,
            "frame state and completion must be the canonical first two fields",
        ));
    }
    let Some(class) = arena_get(&module.classes, frame.class()) else {
        return Err(error(location, "frame refers to an unknown class"));
    };
    let ClassRepresentation::Declared { fields, .. } = &class.representation else {
        return Err(error(
            location,
            "frame class must have a declared representation",
        ));
    };
    if fields.len() != frame.saved_values().len() + 3 {
        return Err(error(
            location,
            "every frame field must have exactly one state, completion, saved, or failure role",
        ));
    }
    Ok(())
}
