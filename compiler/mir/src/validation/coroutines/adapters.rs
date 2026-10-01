use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_resume_point(
    module: &Module,
    point_id: CoroutineResumePointId,
    point: &CoroutineResumePoint,
    frame: &CoroutineFrame,
    driver: &Function,
    coroutine: &CoroutineFunction,
    frame_local: LocalId,
) -> Result<(), MirValidationError> {
    let location = MirValidationLocation::CoroutineResumePoint { point: point_id };
    let Some(adapter) = arena_get(&module.classes, point.adapter()) else {
        return Err(error(
            location,
            "resume point refers to an unknown adapter class",
        ));
    };
    let failure = arena_get(&module.meta.coroutine_failure_values, frame.failure())
        .expect("validated frame failure id remains in bounds");
    let Some((success_signature, failure_signature)) =
        continuation_adapter_signatures(module, adapter, point.result(), failure.throwable())
    else {
        return Err(error(
            location,
            "continuation adapter has no complete exact protocol signatures",
        ));
    };
    let rebuilt_identity = match point.identity().storage() {
        ContinuationAdapterStorageIdentity::Direct => ContinuationAdapterIdentity::direct(
            coroutine.source,
            point.identity().suspension_site().clone(),
            success_signature,
            failure_signature,
            coroutine.source_odr_group,
        ),
        ContinuationAdapterStorageIdentity::Latched { .. } => ContinuationAdapterIdentity::latched(
            coroutine.source,
            point.identity().suspension_site().clone(),
            success_signature,
            failure_signature,
            coroutine.source_odr_group,
        ),
    }
    .map_err(|_| {
        error(
            location,
            "coroutine source cannot form its canonical continuation-adapter identity",
        )
    })?;
    if &rebuilt_identity != point.identity() {
        return Err(error(
            location,
            "continuation-adapter identity does not match its exact source, suspension site, and logical signatures",
        ));
    }
    validate_adapter_fields(module, adapter, point, frame, location)?;
    let Some(resume) = arena_get(&module.functions, point.resume()) else {
        return Err(error(
            location,
            "resume point refers to an unknown success callback",
        ));
    };
    let Some(failure_callback) = arena_get(&module.functions, point.resume_with_exception()) else {
        return Err(error(
            location,
            "resume point refers to an unknown failure callback",
        ));
    };
    if point.failure().exception() != frame.failure() {
        return Err(error(
            location,
            "failure resume entry does not use its owner frame's failure value",
        ));
    }
    if !callback_signature_is(resume, point.adapter(), point.result(), &Type::Unit)
        || !callback_signature_is(
            failure_callback,
            point.adapter(),
            &Type::Class(failure.throwable()),
            &Type::Unit,
        )
    {
        return Err(error(
            location,
            "resume callback signatures do not match adapter, result, and Throwable types",
        ));
    }

    validate_target(driver, point.success().entry().block(), location)?;
    validate_target(driver, point.success().post().block(), location)?;
    validate_target(driver, point.failure().entry().block(), location)?;
    validate_optional_unwind(driver, point.failure().unwind(), location)?;
    for transfer in point.parents() {
        validate_transfer(
            module,
            frame,
            driver,
            &coroutine.source_return,
            failure,
            *transfer,
            location,
        )?;
    }
    let returned: HashSet<_> = point
        .parents()
        .iter()
        .filter_map(|transfer| match transfer {
            CoroutinePendingTransfer::Return(CoroutineReturnTransfer::Saved(value)) => Some(*value),
            _ => None,
        })
        .collect();
    let aliases_throw = point.parents().iter().any(|transfer| match transfer {
        CoroutinePendingTransfer::ManagedThrow(throw_) => returned.contains(&throw_.exception()),
        _ => false,
    });
    if aliases_throw {
        return Err(error(
            location,
            "pending return and managed throw must use distinct saved-value identities",
        ));
    }

    let success_entry = &driver.body.blocks[point.success().entry().block()];
    if !matches!(
        &success_entry.terminator,
        Terminator::Goto(target) if *target == point.success().post().block()
    ) {
        return Err(error(
            location,
            "success resume entry must go directly to its typed post target",
        ));
    }
    let failure_entry = &driver.body.blocks[point.failure().entry().block()];
    let Terminator::Throw { exception, unwind } = &failure_entry.terminator else {
        return Err(error(
            location,
            "failure resume entry must end in a managed throw",
        ));
    };
    let expected_unwind = point.failure().unwind().map(|target| target.block());
    if *unwind != expected_unwind
        || failure_entry.unwind != expected_unwind
        || !reads_failure_value(module, exception, frame, failure, frame_local)
    {
        return Err(error(
            location,
            "failure resume entry must throw its exact frame failure slot along the typed unwind edge",
        ));
    }
    Ok(())
}

fn continuation_adapter_signatures(
    module: &Module,
    adapter: &ClassDef,
    result: &Type,
    throwable: ClassId,
) -> Option<(
    scoop_identity::ExactCallableSignature,
    scoop_identity::ExactCallableSignature,
)> {
    let [continuation] = adapter.interfaces.as_slice() else {
        return None;
    };
    let receiver = source_exact_type(module, &Type::Interface(*continuation))?;
    let result = source_exact_type(module, result)?;
    let throwable = source_exact_type(module, &Type::Class(throwable))?;
    let unit = source_exact_type(module, &Type::Unit)?;
    Some((
        scoop_identity::ExactCallableSignature::new(
            scoop_identity::Effect::Ordinary,
            Some(receiver),
            vec![result],
            unit,
        ),
        scoop_identity::ExactCallableSignature::new(
            scoop_identity::Effect::Ordinary,
            Some(receiver),
            vec![throwable],
            unit,
        ),
    ))
}

fn validate_adapter_fields(
    module: &Module,
    adapter: &ClassDef,
    point: &CoroutineResumePoint,
    frame: &CoroutineFrame,
    location: MirValidationLocation,
) -> Result<(), MirValidationError> {
    let ClassRepresentation::Declared { fields, base_class } = &adapter.representation else {
        return Err(error(
            location,
            "continuation adapter must have a declared class representation",
        ));
    };
    let expected_fields = match point.identity().storage() {
        ContinuationAdapterStorageIdentity::Direct => 2,
        ContinuationAdapterStorageIdentity::Latched { .. } => 4,
    };
    if base_class.is_some()
        || fields.len() != expected_fields
        || fields[0].ty != Type::Class(frame.class())
        || fields[1].ty != Type::MachineScalar(MachineScalarKind::CoroutineAdapterState)
    {
        return Err(error(
            location,
            "continuation adapter does not have its exact frame, state, and latch field layout",
        ));
    }
    if matches!(
        point.identity().storage(),
        ContinuationAdapterStorageIdentity::Latched { .. }
    ) {
        let failure = &module.meta.coroutine_failure_values[frame.failure()];
        let result_slot = coroutine_slot_type(module, point.result());
        let failure_slot = coroutine_slot_type(module, &Type::Class(failure.throwable()));
        if result_slot.as_ref() != Some(&fields[2].ty)
            || failure_slot.as_ref() != Some(&fields[3].ty)
        {
            return Err(error(
                location,
                "latched continuation adapter fields do not use the exact result and Throwable slots",
            ));
        }
    }
    Ok(())
}

fn coroutine_slot_type(module: &Module, value: &Type) -> Option<Type> {
    let mut candidates = module
        .meta
        .coroutine_slots
        .iter()
        .filter(|(_, slot)| slot.value() == value)
        .map(|(_, slot)| Type::Enum(slot.enum_id(), Vec::new()));
    let slot = candidates.next()?;
    candidates.next().is_none().then_some(slot)
}

fn callback_signature_is(
    function: &Function,
    adapter: ClassId,
    value: &Type,
    result: &Type,
) -> bool {
    matches!(
        function.params.as_slice(),
        [receiver, parameter]
            if receiver.ty == Type::Class(adapter) && &parameter.ty == value
    ) && &function.return_ty == result
}

fn reads_failure_value(
    module: &Module,
    expression: &Expr,
    frame: &CoroutineFrame,
    failure: &CoroutineFailureValue,
    frame_local: LocalId,
) -> bool {
    if expression.ty != Type::Class(failure.throwable()) {
        return false;
    }
    let slot = &module.meta.coroutine_slots[failure.slot()];
    let ExprKind::EnumField {
        operand,
        variant,
        index,
    } = &expression.kind
    else {
        return false;
    };
    if *variant != slot.value_payload().variant().variant_index()
        || *index != slot.value_payload().field_index()
        || operand.ty != Type::Enum(slot.enum_id(), Vec::new())
    {
        return false;
    }
    let ExprKind::FieldAccess {
        receiver,
        index: field,
    } = &operand.kind
    else {
        return false;
    };
    *field == failure.field().field_index()
        && receiver.ty == Type::Class(frame.class())
        && matches!(&receiver.kind, ExprKind::Local(local) if *local == frame_local)
}
