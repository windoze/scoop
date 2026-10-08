use super::*;

pub(super) fn validate_coroutine_function(
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
            .any(|exact| !crate::validation::callable_signatures::exact_type_exists(module, exact))
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

    super::signatures::validate(module, coroutine, step, function, location)?;

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
    for adjust in &module.meta.interface_adjusts {
        if adjust.function() == coroutine.function {
            if adjust.identity().signature_record().signature() != &coroutine.logical_signature {
                return Err(error(
                    location,
                    "coroutine logical signature does not match its boxing adapter",
                ));
            }
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
