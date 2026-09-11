//! Producer coverage for typed coroutine pending-transfer chains.

use std::collections::HashSet;

use super::*;

fn suspend_unit(h: &mut Harness, name: &str) -> hir::FunctionId {
    let function = h.user_fn(
        name,
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    h.functions[function].is_suspend = true;
    function
}

fn primary_constructor(h: &Harness, class: hir::ClassId) -> hir::ClassConstructorApplicationId {
    let constructor = h.classes[class].constructors[0];
    h.class_constructor_applications
        .iter()
        .find_map(|(id, application)| (application.constructor == constructor).then_some(id))
        .expect("the exception shell has a primary constructor application")
}

fn class_init(constructor: hir::ClassConstructorApplicationId, ty: hir::TypeId) -> hir::Expr {
    expr(
        hir::ExprKind::ClassInit {
            constructor,
            args: Vec::new(),
        },
        ty,
    )
}

fn state_machine<'a>(
    module: &'a mir::Module,
    name: &str,
) -> (
    mir::CoroutineFrameId,
    &'a mir::CoroutineFrame,
    &'a mir::Function,
    Vec<&'a mir::CoroutineResumePoint>,
) {
    let (owner, coroutine) = module
        .meta
        .coroutine_functions
        .iter()
        .find(|(_, coroutine)| module.functions[coroutine.function].name == name)
        .unwrap_or_else(|| panic!("missing coroutine metadata for `{name}`"));
    let mir::CoroutineLowering::StateMachine {
        frame,
        driver,
        resume_points,
        ..
    } = &coroutine.lowering
    else {
        panic!("`{name}` must lower to a state machine")
    };
    let metadata = &module.meta.coroutine_frames[*frame];
    assert_eq!(metadata.owner(), owner);
    (
        *frame,
        metadata,
        &module.functions[*driver],
        resume_points
            .iter()
            .map(|point| &module.meta.coroutine_resume_points[*point])
            .collect(),
    )
}

fn assert_resume_leaves(
    frame_id: mir::CoroutineFrameId,
    frame: &mir::CoroutineFrame,
    driver: &mir::Function,
    point: &mir::CoroutineResumePoint,
) {
    assert_eq!(point.frame(), frame_id);
    assert_ne!(point.success_state(), point.failure_state());

    let success = point.success();
    assert!(matches!(
        &driver.body.blocks[success.entry().block()].terminator,
        mir::Terminator::Goto(target) if *target == success.post().block()
    ));

    let failure = point.failure();
    assert_eq!(failure.exception(), frame.failure());
    assert!(matches!(
        &driver.body.blocks[failure.entry().block()].terminator,
        mir::Terminator::Throw { unwind, .. }
            if *unwind == failure.unwind().map(|target| target.block())
    ));
}

fn materialized_exception_at(body: &mir::Body, target: mir::CoroutineUnwindTarget) -> mir::LocalId {
    let block = &body.blocks[target.block()];
    let (materialize_index, destination) = block
        .statements
        .iter()
        .enumerate()
        .find_map(|(index, statement)| {
            let mir::StatementKind::Call(mir::CallEffect::Value { destination, call }) =
                &statement.kind
            else {
                return None;
            };
            (call.target.callee == mir::Callee::Runtime(mir::RuntimeFn::MaterializeException))
                .then_some((index, *destination))
        })
        .unwrap_or_else(|| {
            panic!(
                "unwind target `{}` must materialize its exception",
                block.name
            )
        });
    assert!(
        block.statements[..materialize_index]
            .iter()
            .any(|statement| {
                matches!(
                    &statement.kind,
                    mir::StatementKind::Eh(mir::EhStatement::LandingPad { cleanup: false })
                )
            })
    );
    assert!(
        block.statements[..materialize_index]
            .iter()
            .any(|statement| {
                matches!(
                    &statement.kind,
                    mir::StatementKind::Eh(mir::EhStatement::BeginCatch)
                )
            })
    );
    assert!(
        block.statements[materialize_index + 1..]
            .iter()
            .any(|statement| {
                matches!(
                    &statement.kind,
                    mir::StatementKind::Eh(mir::EhStatement::EndCatch)
                )
            })
    );
    destination
}

fn assert_failure_enters_managed_catch(body: &mir::Body, point: &mir::CoroutineResumePoint) {
    let target = point
        .failure()
        .unwind()
        .expect("the suspending call is protected by its inner catch");
    materialized_exception_at(body, target);
    let mir::Terminator::Goto(dispatch) = &body.blocks[target.block()].terminator else {
        panic!("a materialized exception enters typed catch dispatch")
    };
    let mir::Terminator::Branch { then_block, .. } = &body.blocks[*dispatch].terminator else {
        panic!("the inner catch dispatch tests its declared exception type")
    };
    assert!(body.blocks[*then_block].name.starts_with("try.catch"));
}

fn assert_per_scope_exception_storage(body: &mir::Body, minimum: usize) {
    let mut destinations = HashSet::new();
    for (block_id, block) in body.blocks.iter() {
        for statement in &block.statements {
            let mir::StatementKind::Call(mir::CallEffect::Value { destination, call }) =
                &statement.kind
            else {
                continue;
            };
            if call.target.callee != mir::Callee::Runtime(mir::RuntimeFn::MaterializeException)
                || !body.locals[*destination]
                    .name
                    .starts_with("$coroutine_exception")
            {
                continue;
            }
            assert!(
                destinations.insert(*destination),
                "source unwind scope at {block_id:?} must own a fresh exception local"
            );
            let target = mir::CoroutineUnwindTarget::new(block_id);
            assert_eq!(materialized_exception_at(body, target), *destination);
        }
    }
    assert!(
        destinations.len() >= minimum,
        "nested source EH must materialize each scope independently"
    );
}

fn nested_finally_with_caught_suspend(
    h: &Harness,
    suspend: hir::FunctionId,
    caught: hir::LocalId,
    caught_ty: hir::TypeId,
) -> hir::Statement {
    stmt(hir::StatementKind::Try(hir::Try {
        body: Vec::new(),
        catches: Vec::new(),
        finally_body: Some(vec![stmt(hir::StatementKind::Try(hir::Try {
            body: vec![expr_stmt(call(h, suspend, Vec::new()))],
            catches: vec![hir::CatchClause {
                local: caught,
                ty: caught_ty,
                body: Vec::new(),
                span: SPAN,
            }],
            finally_body: None,
        }))]),
    }))
}

#[test]
fn one_suspending_finally_has_distinct_fallthrough_return_and_throw_contexts() {
    let mut h = Harness::new();
    let suspend = suspend_unit(&mut h, "pendingContextSuspend");
    let error = h.exception("PendingContextError");
    let error_ty = h.class_ty(error);
    let error_constructor = primary_constructor(&h, error);
    let boolean = h.boolean;
    let unit = h.unit;
    let mut locals = Arena::new();
    let returns = locals.alloc(local("returns", boolean));
    let throws = locals.alloc(local("throws", boolean));
    let caller = h.user_fn_full(
        "pendingContextCaller",
        Vec::new(),
        vec![
            param("returns", boolean, returns),
            param("throws", boolean, throws),
        ],
        unit,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Try(hir::Try {
                body: vec![
                    stmt(hir::StatementKind::If {
                        cond: local_ref(returns, boolean),
                        then_body: vec![stmt(hir::StatementKind::Return { value: None })],
                        else_body: None,
                    }),
                    stmt(hir::StatementKind::If {
                        cond: local_ref(throws, boolean),
                        then_body: vec![stmt(hir::StatementKind::Throw(class_init(
                            error_constructor,
                            error_ty,
                        )))],
                        else_body: None,
                    }),
                ],
                catches: Vec::new(),
                finally_body: Some(vec![expr_stmt(call(&h, suspend, Vec::new()))]),
            }))],
        },
    );
    h.functions[caller].is_suspend = true;
    let main = empty_main(&mut h);

    let module = lower(&h.finish_coroutines(main));
    assert_eq!(module.validate(), Ok(()));
    let (frame_id, frame, driver, points) = state_machine(&module, "pendingContextCaller");
    assert_eq!(
        points.len(),
        3,
        "one finally is cloned for its three entries"
    );

    let mut states = HashSet::new();
    let mut fallthrough = 0;
    let mut returns = 0;
    let mut throws = 0;
    for point in points {
        assert_resume_leaves(frame_id, frame, driver, point);
        assert!(states.insert(point.success_state()));
        assert!(states.insert(point.failure_state()));
        let [parent] = point.parents() else {
            panic!("each clone of the same finally has one pending parent")
        };
        match parent {
            mir::CoroutinePendingTransfer::Fallthrough(_) => fallthrough += 1,
            mir::CoroutinePendingTransfer::Return(mir::CoroutineReturnTransfer::Unit) => {
                returns += 1
            }
            mir::CoroutinePendingTransfer::ManagedThrow(throw_) => {
                throws += 1;
                let saved = &module.meta.coroutine_saved_values[throw_.exception()];
                let failure = &module.meta.coroutine_failure_values[frame.failure()];
                assert_eq!(saved.value(), &mir::Type::Class(failure.throwable()));
                assert_ne!(saved.field(), failure.field());
            }
            other => panic!("unexpected pending context for the shared finally: {other:?}"),
        }
    }
    assert_eq!((fallthrough, returns, throws), (1, 1, 1));
    assert_eq!(
        states.len(),
        6,
        "success and failure states are site-unique"
    );
}

#[test]
fn nested_finally_catches_site_failure_without_losing_outer_return_or_throw() {
    let mut h = Harness::new();
    let suspend = suspend_unit(&mut h, "nestedPendingSuspend");
    let inner_error = h.exception("NestedPendingInnerError");
    let inner_error_ty = h.class_ty(inner_error);
    let outer_error = h.exception("NestedPendingOuterError");
    let outer_error_ty = h.class_ty(outer_error);
    let outer_error_constructor = primary_constructor(&h, outer_error);
    let int = h.int;

    let mut return_locals = Arena::new();
    let return_caught = return_locals.alloc(local("returnCaught", inner_error_ty));
    let returner = h.user_fn_full(
        "nestedPendingReturn",
        Vec::new(),
        Vec::new(),
        int,
        hir::Body {
            locals: return_locals,
            statements: vec![stmt(hir::StatementKind::Try(hir::Try {
                body: vec![stmt(hir::StatementKind::Return {
                    value: Some(int_lit(&h, 17)),
                })],
                catches: Vec::new(),
                finally_body: Some(vec![nested_finally_with_caught_suspend(
                    &h,
                    suspend,
                    return_caught,
                    inner_error_ty,
                )]),
            }))],
        },
    );
    h.functions[returner].is_suspend = true;

    let mut throw_locals = Arena::new();
    let throw_caught = throw_locals.alloc(local("throwCaught", inner_error_ty));
    let thrower = h.user_fn_full(
        "nestedPendingThrow",
        Vec::new(),
        Vec::new(),
        int,
        hir::Body {
            locals: throw_locals,
            statements: vec![stmt(hir::StatementKind::Try(hir::Try {
                body: vec![stmt(hir::StatementKind::Throw(class_init(
                    outer_error_constructor,
                    outer_error_ty,
                )))],
                catches: Vec::new(),
                finally_body: Some(vec![nested_finally_with_caught_suspend(
                    &h,
                    suspend,
                    throw_caught,
                    inner_error_ty,
                )]),
            }))],
        },
    );
    h.functions[thrower].is_suspend = true;
    let main = empty_main(&mut h);

    let module = lower(&h.finish_coroutines(main));
    assert_eq!(module.validate(), Ok(()));

    let (return_frame_id, return_frame, return_driver, return_points) =
        state_machine(&module, "nestedPendingReturn");
    let return_matches = return_points
        .iter()
        .copied()
        .filter(|point| {
            matches!(
                point.parents(),
                [
                    mir::CoroutinePendingTransfer::Return(mir::CoroutineReturnTransfer::Saved(_)),
                    mir::CoroutinePendingTransfer::Fallthrough(_),
                ]
            )
        })
        .collect::<Vec<_>>();
    let [return_point] = return_matches.as_slice() else {
        panic!("one inner site must retain outer Return then inner Fallthrough")
    };
    assert_resume_leaves(return_frame_id, return_frame, return_driver, return_point);
    assert_failure_enters_managed_catch(&return_driver.body, return_point);
    let [
        mir::CoroutinePendingTransfer::Return(mir::CoroutineReturnTransfer::Saved(returned)),
        mir::CoroutinePendingTransfer::Fallthrough(inner_return),
    ] = return_point.parents()
    else {
        unreachable!()
    };
    assert!(return_frame.owns_saved_value(*returned));
    let returned = &module.meta.coroutine_saved_values[*returned];
    assert_eq!(
        returned.value(),
        &mir::Type::Integer(mir::IntegerKind::SIGNED_32)
    );
    assert_ne!(
        returned.field(),
        module.meta.coroutine_failure_values[return_frame.failure()].field()
    );
    assert_ne!(
        inner_return.block(),
        return_point.success().post().block(),
        "the parent Fallthrough and the site's success leaf are separate chain links"
    );
    assert_per_scope_exception_storage(&return_driver.body, 2);

    let (throw_frame_id, throw_frame, throw_driver, throw_points) =
        state_machine(&module, "nestedPendingThrow");
    let throw_matches = throw_points
        .iter()
        .copied()
        .filter(|point| {
            matches!(
                point.parents(),
                [
                    mir::CoroutinePendingTransfer::ManagedThrow(_),
                    mir::CoroutinePendingTransfer::Fallthrough(_),
                ]
            )
        })
        .collect::<Vec<_>>();
    let [throw_point] = throw_matches.as_slice() else {
        panic!("one inner site must retain outer ManagedThrow then inner Fallthrough")
    };
    assert_resume_leaves(throw_frame_id, throw_frame, throw_driver, throw_point);
    assert_failure_enters_managed_catch(&throw_driver.body, throw_point);
    let [
        mir::CoroutinePendingTransfer::ManagedThrow(thrown),
        mir::CoroutinePendingTransfer::Fallthrough(inner_throw),
    ] = throw_point.parents()
    else {
        unreachable!()
    };
    assert!(throw_frame.owns_saved_value(thrown.exception()));
    let thrown = &module.meta.coroutine_saved_values[thrown.exception()];
    let failure = &module.meta.coroutine_failure_values[throw_frame.failure()];
    assert_eq!(throw_point.failure().exception(), throw_frame.failure());
    assert_eq!(thrown.value(), &mir::Type::Class(failure.throwable()));
    assert_ne!(thrown.field(), failure.field());
    assert_ne!(
        inner_throw.block(),
        throw_point.success().post().block(),
        "the consumed inner failure cannot replace the outer throw continuation"
    );
    assert_per_scope_exception_storage(&throw_driver.body, 2);
}

#[test]
fn synthetic_break_and_continue_keep_distinct_typed_targets_through_suspending_finally() {
    let mut h = Harness::new();
    let suspend = suspend_unit(&mut h, "loopPendingSuspend");
    let boolean = h.boolean;
    let unit = h.unit;
    let target = hir::LoopId::from_raw(0);
    let mut locals = Arena::new();
    let exits = locals.alloc(local("exits", boolean));
    let caller = h.user_fn_full(
        "loopPendingCaller",
        Vec::new(),
        vec![param("exits", boolean, exits)],
        unit,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::While {
                target,
                condition_setup: Vec::new(),
                cond: bool_lit(&h, true),
                body: vec![stmt(hir::StatementKind::Try(hir::Try {
                    body: vec![stmt(hir::StatementKind::If {
                        cond: local_ref(exits, boolean),
                        then_body: vec![stmt(hir::StatementKind::Break { target })],
                        else_body: Some(vec![stmt(hir::StatementKind::Continue { target })]),
                    })],
                    catches: Vec::new(),
                    finally_body: Some(vec![expr_stmt(call(&h, suspend, Vec::new()))]),
                }))],
            })],
        },
    );
    h.functions[caller].is_suspend = true;
    let main = empty_main(&mut h);

    let module = lower(&h.finish_coroutines(main));
    assert_eq!(module.validate(), Ok(()));
    let (_, _, driver, points) = state_machine(&module, "loopPendingCaller");
    let break_targets = points
        .iter()
        .flat_map(|point| point.parents())
        .filter_map(|transfer| match transfer {
            mir::CoroutinePendingTransfer::Break(target) => Some(*target),
            _ => None,
        })
        .collect::<Vec<_>>();
    let continue_targets = points
        .iter()
        .flat_map(|point| point.parents())
        .filter_map(|transfer| match transfer {
            mir::CoroutinePendingTransfer::Continue(target) => Some(*target),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [break_target] = break_targets.as_slice() else {
        panic!("the synthetic break has one typed pending target")
    };
    let [continue_target] = continue_targets.as_slice() else {
        panic!("the synthetic continue has one typed pending target")
    };

    assert_ne!(break_target.block(), continue_target.block());
    assert!(
        driver.body.blocks[break_target.block()]
            .name
            .starts_with("while.exit")
    );
    assert!(
        driver.body.blocks[continue_target.block()]
            .name
            .starts_with("while.cond")
    );
    assert!(
        driver
            .body
            .loop_header_polls
            .iter()
            .any(|poll| poll.header() == continue_target.block())
    );
    assert!(
        driver
            .body
            .loop_header_polls
            .iter()
            .all(|poll| poll.header() != break_target.block())
    );
}
