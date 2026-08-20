use std::collections::{HashMap, HashSet};

use super::*;
use crate::validation::metadata::source_exact_type;

mod adapters;
mod control_flow;
mod frames;
mod functions;
mod signatures;
mod support;

use adapters::validate_resume_point;
use control_flow::{validate_dispatch, validate_transfer};
use frames::{validate_failure_values, validate_frame, validate_saved_values};
use functions::validate_coroutine_function;
use support::validate_support_callables;

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
