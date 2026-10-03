use super::*;

pub(super) fn validate_transfer(
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

pub(super) fn validate_dispatch(
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
