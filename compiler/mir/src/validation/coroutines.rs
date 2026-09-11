use std::collections::{HashMap, HashSet};

use super::*;
use crate::validation::metadata::source_exact_type;

pub(super) fn validate_coroutine_metadata(module: &Module) -> Result<(), MirValidationError> {
    validate_saved_values(module)?;
    validate_failure_values(module)?;
    let mut callables = validate_support_callables(module)?;

    let mut saved_owners = vec![0_u32; module.meta.coroutine_saved_values.len()];
    let mut failure_owners = vec![0_u32; module.meta.coroutine_failure_values.len()];
    let mut frame_classes = HashSet::new();
    let mut frame_identities = HashSet::new();
    for (frame_id, frame) in module.meta.coroutine_frames.iter() {
        validate_frame(module, frame_id, frame)?;
        if !frame_classes.insert(frame.class()) {
            return Err(error(
                MirValidationLocation::CoroutineFrame { frame: frame_id },
                "a generated frame class cannot be shared by two coroutine frames",
            ));
        }
        if !frame_identities.insert(frame.identity().generated_type_record().id()) {
            return Err(error(
                MirValidationLocation::CoroutineFrame { frame: frame_id },
                "a coroutine frame identity must be uniquely materialized",
            ));
        }
        for value in frame.saved_values() {
            saved_owners[raw(*value)] += 1;
        }
        failure_owners[raw(frame.failure())] += 1;
    }
    for (value, _) in module.meta.coroutine_saved_values.iter() {
        if saved_owners[raw(value)] != 1 {
            return Err(error(
                MirValidationLocation::CoroutineSavedValue { value },
                "saved value must belong to exactly one coroutine frame",
            ));
        }
    }
    for (value, _) in module.meta.coroutine_failure_values.iter() {
        if failure_owners[raw(value)] != 1 {
            return Err(error(
                MirValidationLocation::CoroutineFailureValue { value },
                "failure value must belong to exactly one coroutine frame",
            ));
        }
    }

    let mut frame_owners = vec![0_u32; module.meta.coroutine_frames.len()];
    let mut point_owners = vec![0_u32; module.meta.coroutine_resume_points.len()];
    let mut drivers = HashSet::new();
    let mut driver_identities = HashSet::new();
    let mut adapter_classes = HashSet::new();
    let mut adapter_identities = HashSet::new();
    let mut adapter_callable_identities = HashSet::new();
    for (point_id, point) in module.meta.coroutine_resume_points.iter() {
        let location = MirValidationLocation::CoroutineResumePoint { point: point_id };
        if !adapter_classes.insert(point.adapter()) {
            return Err(error(
                location,
                "a continuation-adapter class must be uniquely owned",
            ));
        }
        if !adapter_identities.insert(point.identity().generated_type_record().id()) {
            return Err(error(
                location,
                "a continuation-adapter environment identity must be uniquely materialized",
            ));
        }
        for (function, identity) in [
            (point.resume(), point.identity().success()),
            (point.resume_with_exception(), point.identity().failure()),
        ] {
            if !callables.insert(function) {
                return Err(error(
                    location,
                    "a continuation-adapter callback must be distinct and uniquely owned",
                ));
            }
            if !adapter_callable_identities.insert(identity.callable_record().id()) {
                return Err(error(
                    location,
                    "a continuation-adapter callable identity must be uniquely materialized",
                ));
            }
        }
    }
    for (coroutine_id, coroutine) in module.meta.coroutine_functions.iter() {
        if !callables.insert(coroutine.function) {
            return Err(error(
                MirValidationLocation::CoroutineFunction {
                    coroutine: coroutine_id,
                },
                "a suspend callable cannot be owned by two coroutine records",
            ));
        }
        if let CoroutineLowering::StateMachine {
            driver,
            driver_identity,
            ..
        } = &coroutine.lowering
        {
            if coroutine.function == *driver
                || !drivers.insert(*driver)
                || !callables.insert(*driver)
            {
                return Err(error(
                    MirValidationLocation::CoroutineFunction {
                        coroutine: coroutine_id,
                    },
                    "a coroutine driver must be distinct and uniquely owned",
                ));
            }
            if !driver_identities.insert(driver_identity.callable_record().id()) {
                return Err(error(
                    MirValidationLocation::CoroutineFunction {
                        coroutine: coroutine_id,
                    },
                    "a coroutine driver identity must be uniquely materialized",
                ));
            }
        }
        validate_coroutine_function(
            module,
            coroutine_id,
            coroutine,
            &mut frame_owners,
            &mut point_owners,
        )?;
    }
    for (frame, _) in module.meta.coroutine_frames.iter() {
        if frame_owners[raw(frame)] != 1 {
            return Err(error(
                MirValidationLocation::CoroutineFrame { frame },
                "frame must belong to exactly one state-machine coroutine",
            ));
        }
    }
    for (point, _) in module.meta.coroutine_resume_points.iter() {
        if point_owners[raw(point)] != 1 {
            return Err(error(
                MirValidationLocation::CoroutineResumePoint { point },
                "resume point must occur exactly once in its owner's point list",
            ));
        }
    }
    Ok(())
}

fn validate_support_callables(module: &Module) -> Result<HashSet<FunctionId>, MirValidationError> {
    let mut functions = HashSet::new();
    let mut shell_results = HashSet::new();
    for (index, shell) in module.meta.continuation_shells.iter().enumerate() {
        let location = MirValidationLocation::ContinuationShell {
            shell: u32::try_from(index).expect("MIR metadata index fits u32"),
        };
        if CoroutineContinuationShell::checked(
            &module.functions,
            shell.result().clone(),
            shell.success(),
            shell.failure(),
            shell.identity().clone(),
        )
        .is_none()
        {
            return Err(error(
                location,
                "continuation shells no longer have their exact generated signatures",
            ));
        }
        let exact = shell.identity().result_record().id();
        if !shell_results.insert(exact) {
            return Err(error(
                location,
                "an exact result can have only one continuation-shell pair",
            ));
        }
        if step_exact_result(module, shell.result()) != Some(exact) {
            return Err(error(
                location,
                "continuation shells and CoroutineStep disagree on the exact result",
            ));
        }
        if source_exact_type(module, shell.result()) != Some(exact) {
            return Err(error(
                location,
                "continuation shell result does not match the source exact-type relation",
            ));
        }
        let Some(success_signature) = exact_support_signature(module, shell.success(), true) else {
            return Err(error(
                location,
                "continuation success signature has no complete exact-type relation",
            ));
        };
        let Some(failure_signature) = exact_support_signature(module, shell.failure(), true) else {
            return Err(error(
                location,
                "continuation failure signature has no complete exact-type relation",
            ));
        };
        if shell.identity().success_signature_record().signature() != &success_signature
            || shell.identity().failure_signature_record().signature() != &failure_signature
        {
            return Err(error(
                location,
                "continuation shell identity does not retain its exact logical signatures",
            ));
        }
        if !functions.insert(shell.success()) || !functions.insert(shell.failure()) {
            return Err(error(
                location,
                "each continuation shell function must be uniquely owned",
            ));
        }
    }

    let mut start_results = HashSet::new();
    for (index, start) in module.meta.coroutine_starts.iter().enumerate() {
        let location = MirValidationLocation::CoroutineStart {
            start: u32::try_from(index).expect("MIR metadata index fits u32"),
        };
        if CoroutineStart::checked(
            &module.functions,
            start.result().clone(),
            start.function(),
            start.identity().clone(),
        )
        .is_none()
        {
            return Err(error(
                location,
                "coroutine start helper no longer has its exact erased signature",
            ));
        }
        let exact = start.identity().result_record().id();
        if !start_results.insert(exact) {
            return Err(error(
                location,
                "an exact result can have only one coroutine start helper",
            ));
        }
        if step_exact_result(module, start.result()) != Some(exact) {
            return Err(error(
                location,
                "coroutine start helper and CoroutineStep disagree on the exact result",
            ));
        }
        if source_exact_type(module, start.result()) != Some(exact) {
            return Err(error(
                location,
                "coroutine start result does not match the source exact-type relation",
            ));
        }
        let Some(signature) = exact_support_signature(module, start.function(), false) else {
            return Err(error(
                location,
                "coroutine start signature has no complete exact-type relation",
            ));
        };
        if start.identity().signature_record().signature() != &signature {
            return Err(error(
                location,
                "coroutine start identity does not retain its exact logical signature",
            ));
        }
        if !functions.insert(start.function()) {
            return Err(error(
                location,
                "each coroutine support function must be uniquely owned",
            ));
        }
    }
    Ok(functions)
}

fn exact_support_signature(
    module: &Module,
    function: FunctionId,
    has_receiver: bool,
) -> Option<scoop_identity::ExactCallableSignature> {
    let function = arena_get(&module.functions, function)?;
    let mut parameters = function.params.iter();
    let receiver = has_receiver
        .then(|| source_exact_type(module, &parameters.next()?.ty))
        .flatten();
    if has_receiver && receiver.is_none() {
        return None;
    }
    let parameters = parameters
        .map(|parameter| source_exact_type(module, &parameter.ty))
        .collect::<Option<Vec<_>>>()?;
    let result = source_exact_type(module, &function.return_ty)?;
    Some(scoop_identity::ExactCallableSignature::new(
        scoop_identity::Effect::Ordinary,
        receiver,
        parameters,
        result,
    ))
}

fn step_exact_result(
    module: &Module,
    result: &Type,
) -> Option<scoop_identity::PersistentExactTypeId> {
    let mut matching = module.meta.coroutine_steps.iter().filter_map(|(_, step)| {
        (step.result() == result).then_some(step.identity().result_record().id())
    });
    let exact = matching.next()?;
    matching.next().is_none().then_some(exact)
}

fn validate_saved_values(module: &Module) -> Result<(), MirValidationError> {
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

fn validate_failure_values(module: &Module) -> Result<(), MirValidationError> {
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

fn validate_frame(
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

fn validate_coroutine_function(
    module: &Module,
    coroutine_id: CoroutineFunctionId,
    coroutine: &CoroutineFunction,
    frame_owners: &mut [u32],
    point_owners: &mut [u32],
) -> Result<(), MirValidationError> {
    let location = MirValidationLocation::CoroutineFunction {
        coroutine: coroutine_id,
    };
    let Some(function) = arena_get(&module.functions, coroutine.function) else {
        return Err(error(location, "coroutine refers to an unknown callable"));
    };
    validate_coroutine_source(module, coroutine, location)?;
    if coroutine.logical_signature.effect() != scoop_identity::Effect::Suspend
        || source_exact_type(module, &coroutine.source_return)
            != Some(coroutine.logical_signature.result())
        || coroutine
            .logical_signature
            .receiver()
            .into_option()
            .into_iter()
            .chain(coroutine.logical_signature.parameters().iter().copied())
            .chain(std::iter::once(coroutine.logical_signature.result()))
            .any(|exact| {
                module
                    .meta
                    .source_exact_types
                    .get_by_identity(exact)
                    .is_none()
            })
    {
        return Err(error(
            location,
            "coroutine source has no exact suspend logical signature",
        ));
    }
    if module
        .meta
        .source_callable_materializations
        .get(coroutine.function)
        .is_some_and(|source| source.signature_record().signature() != &coroutine.logical_signature)
    {
        return Err(error(
            location,
            "coroutine logical signature does not match its source callable materialization",
        ));
    }
    let Some(step) = arena_get(&module.meta.coroutine_steps, coroutine.step) else {
        return Err(error(
            location,
            "coroutine refers to an unknown CoroutineStep",
        ));
    };
    if step.result() != &coroutine.source_return
        || function.return_ty != Type::Enum(step.enum_id(), Vec::new())
    {
        return Err(error(
            location,
            "callable result, source result, and CoroutineStep payload are not exact",
        ));
    }

    let CoroutineLowering::StateMachine {
        frame,
        driver,
        driver_identity,
        resume_points,
    } = &coroutine.lowering
    else {
        return Ok(());
    };
    let Some(frame_metadata) = arena_get(&module.meta.coroutine_frames, *frame) else {
        return Err(error(location, "state machine refers to an unknown frame"));
    };
    if frame_metadata.owner() != coroutine_id {
        return Err(error(location, "state machine frame has a different owner"));
    }
    frame_owners[raw(*frame)] += 1;
    let Some(driver_function) = arena_get(&module.functions, *driver) else {
        return Err(error(location, "state machine refers to an unknown driver"));
    };
    let rebuilt_identity = CoroutineDriverIdentity::new(
        coroutine.source,
        coroutine.source_odr_group,
        coroutine.logical_signature.clone(),
    )
    .map_err(|_| {
        error(
            location,
            "coroutine source cannot form its canonical driver identity",
        )
    })?;
    if &rebuilt_identity != driver_identity.as_ref() {
        return Err(error(
            location,
            "coroutine driver identity does not match its exact source materialization",
        ));
    }
    let [frame_parameter, state_parameter] = driver_function.params.as_slice() else {
        return Err(error(
            location,
            "coroutine driver must have exactly frame and state parameters",
        ));
    };
    if frame_parameter.ty != Type::Class(frame_metadata.class())
        || state_parameter.ty != Type::MachineScalar(MachineScalarKind::CoroutineFrameState)
        || driver_function.return_ty != Type::Enum(step.enum_id(), Vec::new())
    {
        return Err(error(
            location,
            "coroutine driver has a non-exact frame, state, or CoroutineStep signature",
        ));
    }
    let Some(completion_parameter) = function.params.last() else {
        return Err(error(
            location,
            "state-machine wrapper has no hidden completion parameter",
        ));
    };
    if frame_metadata
        .completion()
        .definition(&module.classes)
        .is_none_or(|field| field.ty != completion_parameter.ty)
    {
        return Err(error(
            location,
            "frame completion field does not match the hidden completion parameter",
        ));
    }
    if resume_points.is_empty() {
        return Err(error(
            location,
            "a state-machine coroutine must have at least one resume point",
        ));
    }

    let mut listed = HashSet::new();
    let mut sites = HashSet::new();
    let mut identity_sites = HashSet::new();
    for point_id in resume_points {
        if !listed.insert(*point_id) {
            return Err(error(
                location,
                "state-machine resume-point list contains a duplicate id",
            ));
        }
        let Some(point) = arena_get(&module.meta.coroutine_resume_points, *point_id) else {
            return Err(error(
                location,
                "state-machine resume-point list contains an unknown id",
            ));
        };
        if point.frame() != *frame {
            return Err(error(
                MirValidationLocation::CoroutineResumePoint { point: *point_id },
                "resume point belongs to a different frame",
            ));
        }
        if !sites.insert(point.site()) {
            return Err(error(
                MirValidationLocation::CoroutineResumePoint { point: *point_id },
                "suspension site is reused by another pending context",
            ));
        }
        if !identity_sites.insert(point.identity().suspension_site()) {
            return Err(error(
                MirValidationLocation::CoroutineResumePoint { point: *point_id },
                "a structural suspension site is reused by another continuation adapter",
            ));
        }
        point_owners[raw(*point_id)] += 1;
        validate_resume_point(
            module,
            *point_id,
            point,
            frame_metadata,
            driver_function,
            coroutine,
            frame_parameter.local,
        )?;
    }
    validate_dispatch(
        *driver,
        driver_function,
        state_parameter.local,
        resume_points,
        module,
    )
}

fn validate_coroutine_source(
    module: &Module,
    coroutine: &CoroutineFunction,
    location: MirValidationLocation,
) -> Result<(), MirValidationError> {
    let mut candidates = Vec::new();
    if let Some(source) = module
        .meta
        .source_callable_materializations
        .get(coroutine.function)
    {
        let group = match source.materialization().context() {
            scoop_identity::CallableMaterializationContext::NoSubstitution => None,
            scoop_identity::CallableMaterializationContext::Application(_)
            | scoop_identity::CallableMaterializationContext::InitializationApplication(_) => {
                coroutine.source_odr_group
            }
        };
        candidates.push((source.materialization(), group));
    }
    for (_, adapter) in module.meta.closure_adapters.iter() {
        if closure_invoke_function(module, adapter.class()) == Some(coroutine.function) {
            candidates.push((
                adapter.identity().materialization(),
                Some(adapter.identity().odr_group_record().id()),
            ));
        }
    }
    for (_, adapter) in module.meta.dynamic_closure_adapters.iter() {
        if closure_invoke_function(module, adapter.class()) == Some(coroutine.function) {
            candidates.push((
                adapter.identity().materialization(),
                Some(adapter.identity().odr_group_record().id()),
            ));
        }
    }
    for bridge in &module.meta.function_bridges {
        if bridge.function() == coroutine.function {
            candidates.push((
                bridge.identity().materialization(),
                bridge
                    .identity()
                    .odr_member_record()
                    .map(|member| member.key().group()),
            ));
        }
    }
    for adjust in &module.meta.boxing_adjusts {
        if adjust.function() == coroutine.function {
            candidates.push((
                adjust.identity().materialization(),
                adjust
                    .identity()
                    .root()
                    .member_record()
                    .map(|member| member.key().group()),
            ));
        }
    }

    let [(materialization, group)] = candidates.as_slice() else {
        return Err(error(
            location,
            "a coroutine source must have exactly one typed callable identity",
        ));
    };
    if *materialization != coroutine.source || *group != coroutine.source_odr_group {
        return Err(error(
            location,
            "coroutine source identity or materialization root does not match its callable",
        ));
    }
    Ok(())
}

fn closure_invoke_function(module: &Module, class: ClosureClassId) -> Option<FunctionId> {
    let class = arena_get(&module.closure_classes, class)?;
    let invoke = arena_get(&module.closure_invoke_functions, class.invoke)?;
    Some(invoke.function)
}

#[allow(clippy::too_many_arguments)]
fn validate_resume_point(
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

fn validate_transfer(
    module: &Module,
    frame: &CoroutineFrame,
    driver: &Function,
    source_return: &Type,
    failure: &CoroutineFailureValue,
    transfer: CoroutinePendingTransfer,
    location: MirValidationLocation,
) -> Result<(), MirValidationError> {
    match transfer {
        CoroutinePendingTransfer::Fallthrough(target) => {
            validate_target(driver, target.block(), location)
        }
        CoroutinePendingTransfer::Break(target) => {
            validate_target(driver, target.block(), location)
        }
        CoroutinePendingTransfer::Continue(target) => {
            validate_target(driver, target.block(), location)?;
            if !driver
                .body
                .loop_header_polls
                .iter()
                .any(|poll| poll.header() == target.block())
            {
                return Err(error(
                    location,
                    "pending continue target is not an explicit loop-header poll target",
                ));
            }
            Ok(())
        }
        CoroutinePendingTransfer::Return(return_) => match return_ {
            CoroutineReturnTransfer::Unit if source_return == &Type::Unit => Ok(()),
            CoroutineReturnTransfer::Saved(value) if source_return != &Type::Unit => {
                let Some(value_metadata) = arena_get(&module.meta.coroutine_saved_values, value)
                else {
                    return Err(error(
                        location,
                        "pending return refers to an unknown saved value",
                    ));
                };
                if !frame.owns_saved_value(value) || value_metadata.value() != source_return {
                    return Err(error(
                        location,
                        "pending return must use an owner-frame slot of the exact source result type",
                    ));
                }
                Ok(())
            }
            CoroutineReturnTransfer::Unit | CoroutineReturnTransfer::Saved(_) => Err(error(
                location,
                "Return(Unit) is valid only for an exact Unit source result",
            )),
        },
        CoroutinePendingTransfer::ManagedThrow(throw_) => {
            let Some(value) = arena_get(&module.meta.coroutine_saved_values, throw_.exception())
            else {
                return Err(error(
                    location,
                    "pending managed throw refers to an unknown saved value",
                ));
            };
            if !frame.owns_saved_value(throw_.exception())
                || value.value() != &Type::Class(failure.throwable())
            {
                return Err(error(
                    location,
                    "pending managed throw must use an owner-frame slot of exact Throwable type",
                ));
            }
            validate_optional_unwind(driver, throw_.unwind(), location)
        }
    }
}

fn validate_dispatch(
    driver_id: FunctionId,
    driver: &Function,
    state_local: LocalId,
    points: &[CoroutineResumePointId],
    module: &Module,
) -> Result<(), MirValidationError> {
    let location = points
        .first()
        .copied()
        .map(|point| MirValidationLocation::CoroutineResumePoint { point })
        .unwrap_or(MirValidationLocation::FunctionBlock {
            function: driver_id,
            block: driver.body.entry,
        });
    let mut expected = HashMap::new();
    for point_id in points {
        let point = &module.meta.coroutine_resume_points[*point_id];
        expected.insert(point.success_state(), point.success().entry().block());
        expected.insert(point.failure_state(), point.failure().entry().block());
    }
    let mut found = HashMap::new();
    let mut current = driver.body.entry;
    let mut visited = HashSet::new();
    while visited.insert(current) {
        let Some(block) = arena_get(&driver.body.blocks, current) else {
            return Err(error(location, "driver dispatch reaches an unknown block"));
        };
        if !block.statements.is_empty() || block.unwind.is_some() {
            break;
        }
        let Terminator::Branch {
            cond,
            then_block,
            else_block,
        } = &block.terminator
        else {
            break;
        };
        let Some(state) = dispatch_state(cond, state_local) else {
            break;
        };
        if found.insert(state, *then_block).is_some() {
            return Err(error(
                location,
                "driver dispatch contains a duplicate state",
            ));
        }
        current = *else_block;
    }
    if found.remove(&CoroutineFrameState::Initial).is_none() {
        return Err(error(location, "driver dispatch has no initial-state case"));
    }
    if found != expected {
        return Err(error(
            location,
            "driver dispatch cases do not exactly match resume-point metadata",
        ));
    }
    Ok(())
}

fn dispatch_state(condition: &Expr, state_local: LocalId) -> Option<CoroutineFrameState> {
    let ExprKind::Binary {
        op: BinOp::MachineEq(MachineScalarKind::CoroutineFrameState),
        lhs,
        rhs,
    } = &condition.kind
    else {
        return None;
    };
    if !matches!(&lhs.kind, ExprKind::Local(local) if *local == state_local) {
        return None;
    }
    let ExprKind::MachineScalarLiteral(MachineScalarValue::CoroutineFrameState(state)) = &rhs.kind
    else {
        return None;
    };
    Some(*state)
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

fn validate_target(
    driver: &Function,
    target: BlockId,
    location: MirValidationLocation,
) -> Result<(), MirValidationError> {
    if arena_get(&driver.body.blocks, target).is_none() {
        Err(error(
            location,
            "typed continuation target is outside the driver body",
        ))
    } else {
        Ok(())
    }
}

fn validate_optional_unwind(
    driver: &Function,
    target: Option<CoroutineUnwindTarget>,
    location: MirValidationLocation,
) -> Result<(), MirValidationError> {
    if let Some(target) = target {
        validate_target(driver, target.block(), location)?;
    }
    Ok(())
}

fn error(location: MirValidationLocation, reason: &'static str) -> MirValidationError {
    MirValidationError {
        location,
        kind: MirValidationErrorKind::InvalidCoroutineMetadata { reason },
    }
}

fn raw<T>(id: la_arena::Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

fn arena_get<T>(arena: &Arena<T>, id: Idx<T>) -> Option<&T> {
    (raw(id) < arena.len()).then(|| &arena[id])
}
