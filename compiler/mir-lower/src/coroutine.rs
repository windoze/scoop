//! Typed coroutine state-machine construction over normalized MIR CFG.

use std::collections::{HashMap, HashSet};

use la_arena::Arena;
use scoop_ast::Span;
use scoop_hir::concrete as hir;
use scoop_mir as mir;

use super::Lowerer;

mod eh;

#[derive(Clone)]
struct SuspendSite {
    block: mir::BlockId,
    statement: usize,
    destination: Option<mir::LocalId>,
    result: mir::Type,
    kind: SuspendKind,
    live_after: Vec<mir::LocalId>,
    state: u32,
}

#[derive(Clone, Copy)]
enum SuspendKind {
    Call,
    Intrinsic {
        register: mir::MonomorphizedFunctionId,
    },
}

pub(super) fn transform(lowerer: &mut Lowerer, module: &hir::Module) {
    let coroutine_ids: Vec<_> = lowerer
        .coroutines
        .functions
        .iter()
        .map(|(id, _)| id)
        .collect();
    for coroutine in coroutine_ids {
        let function = lowerer.coroutines.functions[coroutine].function;
        let mut sites = analyze_sites(lowerer, &lowerer.functions[function].body);
        if sites.is_empty() {
            continue;
        }
        let throwable =
            mir::Type::Class(lowerer.class_map[&module.exception_core.throwable.class()]);
        eh::materialize_exceptions(&mut lowerer.functions[function].body, throwable);
        sites = analyze_sites(lowerer, &lowerer.functions[function].body);
        for (index, site) in sites.iter_mut().enumerate() {
            site.state = index as u32 + 1;
        }
        transform_function(lowerer, module, coroutine, sites);
    }
}

fn transform_function(
    lowerer: &mut Lowerer,
    module: &hir::Module,
    coroutine: mir::CoroutineFunctionId,
    mut sites: Vec<SuspendSite>,
) {
    let coroutine_meta = &lowerer.coroutines.functions[coroutine];
    let function_id = coroutine_meta.function;
    let source_return = coroutine_meta.source_return.clone();
    let step_ty = lowerer.functions[function_id].return_ty.clone();
    let (source_name, source_symbol, old_params, mut body) = {
        let function = &mut lowerer.functions[function_id];
        (
            function.name.clone(),
            function.symbol.clone(),
            std::mem::take(&mut function.params),
            std::mem::replace(&mut function.body, mir::Body::unreachable(Arena::new())),
        )
    };
    let completion = old_params
        .last()
        .expect("hidden coroutine ABI always has a completion parameter");
    let completion_old = completion.local;
    let completion_ty = completion.ty.clone();
    let source_params = &old_params[..old_params.len() - 1];
    let throwable_ty =
        mir::Type::Class(lowerer.class_map[&module.exception_core.throwable.class()]);

    let mut saved = HashSet::new();
    saved.extend(source_params.iter().map(|param| param.local));
    for site in &sites {
        saved.extend(
            site.live_after
                .iter()
                .copied()
                .filter(|local| *local != completion_old),
        );
        if let Some(destination) = site.destination {
            saved.insert(destination);
        }
    }
    let mut saved: Vec<_> = saved.into_iter().collect();
    saved.sort_by_key(|local| raw(*local));

    let mut frame_fields = vec![
        mir::Field {
            name: "state".to_string(),
            ty: mir::Type::Int,
        },
        mir::Field {
            name: "completion".to_string(),
            ty: completion_ty.clone(),
        },
    ];
    let mut frame_slots = HashMap::new();
    for local in &saved {
        let value_ty = body.locals[*local].ty.clone();
        let (_, slot_ty) = lowerer.coroutines.slot_for(
            &value_ty,
            &lowerer.structs,
            &mut lowerer.enums,
            &mut lowerer.shell,
        );
        let field = frame_fields.len() as u32;
        frame_fields.push(mir::Field {
            name: format!("local${}", body.locals[*local].name),
            ty: slot_ty.clone(),
        });
        frame_slots.insert(
            *local,
            FrameSlot {
                field,
                slot_ty: slot_ty.clone(),
                value_ty,
            },
        );
    }
    let (_, failure_slot_ty) = lowerer.coroutines.slot_for(
        &throwable_ty,
        &lowerer.structs,
        &mut lowerer.enums,
        &mut lowerer.shell,
    );
    let failure_slot = FrameSlot {
        field: frame_fields.len() as u32,
        slot_ty: failure_slot_ty.clone(),
        value_ty: throwable_ty.clone(),
    };
    frame_fields.push(mir::Field {
        name: "failure".to_string(),
        ty: failure_slot_ty,
    });
    let frame_name = format!("CoroutineFrame${}", sanitize(&source_symbol));
    let frame_class = generated_class(lowerer, frame_name, frame_fields, Vec::new(), Vec::new());
    let frame = lowerer.coroutines.frames.alloc(mir::CoroutineFrame {
        class: frame_class,
        owner: coroutine,
    });

    let driver = lowerer.functions.alloc(mir::Function {
        gc_effect: mir::GcEffect::Managed,
        name: format!("{source_name}$drive"),
        symbol: format!("{source_symbol}$drive"),
        params: Vec::new(),
        return_ty: step_ty.clone(),
        body: mir::Body::unreachable(Arena::new()),
    });
    lowerer.top_level.push(driver);
    let frame_local = body.locals.alloc(mir::Local {
        name: "$frame".to_string(),
        ty: mir::Type::Class(frame_class),
        mutable: false,
    });
    let dispatch_state = body.locals.alloc(mir::Local {
        name: "$dispatch_state".to_string(),
        ty: mir::Type::Int,
        mutable: false,
    });

    let continuation = match completion_ty {
        mir::Type::Interface(interface) => interface,
        _ => unreachable!("hidden completion has a concrete Continuation<R> type"),
    };
    let (outer_resume, outer_failure) = lowerer.coroutines.continuation_shells(
        &source_return,
        continuation,
        throwable_ty,
        &mut lowerer.functions,
        &lowerer.shell,
    );

    sites.sort_by_key(|site| (raw(site.block), site.statement));
    sites.reverse();
    let mut resume_targets = Vec::new();
    let mut resume_points = Vec::new();
    for site in sites {
        let generated = rewrite_site(
            lowerer,
            module,
            &mut body,
            frame_local,
            frame_class,
            frame,
            &frame_slots,
            failure_slot.clone(),
            &step_ty,
            continuation,
            outer_resume,
            outer_failure,
            &source_symbol,
            driver,
            site,
        );
        resume_targets.push((generated.state, generated.resume_block));
        resume_targets.push((generated.failure_state, generated.failure_block));
        resume_points.push(generated.point);
    }
    resume_targets.sort_by_key(|(state, _)| *state);
    resume_points.sort_by_key(|point| lowerer.coroutines.resume_points[*point].state);

    let original_entry = body.entry;
    let initial = body.blocks.alloc(mir::BasicBlock {
        name: "coroutine.initial".to_string(),
        statements: source_params
            .iter()
            .map(|param| {
                restore_statement(
                    mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                    param.local,
                    &frame_slots[&param.local],
                )
            })
            .chain(std::iter::once(atomic_field_store(
                mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                0,
                mir::Expr::int(STATE_RUNNING),
            )))
            .collect(),
        terminator: mir::Terminator::Goto(original_entry),
        unwind: None,
    });
    let invalid = protocol_error_block(lowerer, module, &mut body.locals, &mut body.blocks, None);
    let mut next = invalid;
    for (state, target) in resume_targets.iter().rev() {
        next = dispatch_block(&mut body.blocks, dispatch_state, *state, *target, next);
    }
    let entry = dispatch_block(
        &mut body.blocks,
        dispatch_state,
        STATE_INITIAL,
        initial,
        next,
    );
    body.entry = entry;

    lowerer.functions[driver].params = vec![
        mir::Param {
            name: "$frame".to_string(),
            ty: mir::Type::Class(frame_class),
            local: frame_local,
        },
        mir::Param {
            name: "$dispatch_state".to_string(),
            ty: mir::Type::Int,
            local: dispatch_state,
        },
    ];
    lowerer.functions[driver].body = body;

    let (wrapper_params, wrapper_locals, wrapper_param_map, wrapper_completion) =
        wrapper_params(&old_params);
    lowerer.functions[function_id].params = wrapper_params;
    lowerer.functions[function_id].body = wrapper_body(
        frame_class,
        wrapper_locals,
        &saved,
        &frame_slots,
        &wrapper_param_map,
        wrapper_completion,
        failure_slot,
        driver,
        &step_ty,
    );
    lowerer.coroutines.functions[coroutine].lowering = mir::CoroutineLowering::StateMachine {
        frame,
        driver,
        resume_points,
    };
}

const STATE_INITIAL: i64 = 0;
const STATE_RUNNING: i64 = -1;
const STATE_COMPLETED: i64 = -2;
const ADAPTER_REGISTERING: i64 = 0;
const ADAPTER_WAITING: i64 = 1;
const ADAPTER_COMPLETING_SUCCESS: i64 = 2;
const ADAPTER_COMPLETING_FAILURE: i64 = 3;
const ADAPTER_LATCHED_SUCCESS: i64 = 4;
const ADAPTER_LATCHED_FAILURE: i64 = 5;
const ADAPTER_CONSUMED: i64 = 6;

fn failure_state(state: u32) -> i64 {
    -i64::from(state) - 2
}

#[derive(Clone)]
struct FrameSlot {
    field: u32,
    slot_ty: mir::Type,
    value_ty: mir::Type,
}

struct GeneratedSite {
    state: i64,
    resume_block: mir::BlockId,
    failure_state: i64,
    failure_block: mir::BlockId,
    point: mir::CoroutineResumePointId,
}

#[allow(clippy::too_many_arguments)]
fn rewrite_site(
    lowerer: &mut Lowerer,
    module: &hir::Module,
    body: &mut mir::Body,
    frame_local: mir::LocalId,
    frame_class: mir::ClassId,
    frame: mir::CoroutineFrameId,
    frame_slots: &HashMap<mir::LocalId, FrameSlot>,
    failure_slot: FrameSlot,
    outer_step: &mir::Type,
    outer_continuation: mir::InterfaceId,
    outer_resume: mir::FunctionId,
    outer_failure: mir::FunctionId,
    source_symbol: &str,
    driver: mir::FunctionId,
    site: SuspendSite,
) -> GeneratedSite {
    let block = &mut body.blocks[site.block];
    let suffix = block.statements.split_off(site.statement + 1);
    let suspend_statement = block
        .statements
        .pop()
        .expect("suspend site statement exists");
    let terminator = std::mem::replace(&mut block.terminator, mir::Terminator::Unreachable);
    let unwind = block.unwind;
    let post = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.post.{}", site.state),
        statements: suffix,
        terminator,
        unwind,
    });

    let (mut call, destination) = match suspend_statement.kind {
        mir::StatementKind::Call(mir::CallEffect::Unit(call)) => (call, None),
        mir::StatementKind::Call(mir::CallEffect::Value { destination, call }) => {
            (call, Some(destination))
        }
        _ => unreachable!("suspend site is an explicit call effect"),
    };
    assert_eq!(destination, site.destination);
    let SuspendKind::Call = site.kind else {
        return rewrite_intrinsic_site(
            lowerer,
            module,
            body,
            frame_local,
            frame_class,
            frame,
            frame_slots,
            failure_slot.clone(),
            outer_step,
            outer_continuation,
            outer_resume,
            outer_failure,
            source_symbol,
            driver,
            site,
            call,
            post,
            unwind,
        );
    };

    let adapter = generate_adapter(
        lowerer,
        module,
        frame_class,
        frame,
        destination.map(|local| frame_slots[&local].clone()),
        failure_slot.clone(),
        outer_step,
        outer_continuation,
        outer_resume,
        outer_failure,
        source_symbol,
        driver,
        site.state,
        failure_state(site.state),
        &site.result,
        None,
    );
    let adapter_local = body.locals.alloc(mir::Local {
        name: format!("$adapter.{}", site.state),
        ty: mir::Type::Class(adapter.class),
        mutable: false,
    });
    let step_ty = callee_return_type(lowerer, call.target.callee);
    let step_local = body.locals.alloc(mir::Local {
        name: format!("$step.{}", site.state),
        ty: step_ty.clone(),
        mutable: false,
    });

    let block = &mut body.blocks[site.block];
    for local in site
        .live_after
        .iter()
        .copied()
        .filter(|local| Some(*local) != destination)
    {
        if let Some(slot) = frame_slots.get(&local) {
            block.statements.push(save_statement(
                mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                mir::Expr::local(local, body.locals[local].ty.clone()),
                slot,
            ));
        }
    }
    block.statements.push(atomic_field_store(
        mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
        0,
        mir::Expr::int(i64::from(site.state)),
    ));
    block
        .statements
        .push(statement(mir::StatementKind::ValDecl {
            local: adapter_local,
            init: mir::Expr::new(
                mir::Type::Class(adapter.class),
                mir::ExprKind::ClassInit {
                    class_id: adapter.class,
                    args: vec![
                        mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                        mir::Expr::int(ADAPTER_WAITING),
                    ],
                },
            ),
        }));
    call.args.push(mir::Expr::local(
        adapter_local,
        mir::Type::Class(adapter.class),
    ));
    block.statements.push(statement(mir::StatementKind::Call(
        mir::CallEffect::Value {
            destination: step_local,
            call,
        },
    )));

    let completed = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.completed.{}", site.state),
        statements: {
            let mut statements = Vec::new();
            if let Some(destination) = destination {
                statements.push(statement(mir::StatementKind::ValDecl {
                    local: destination,
                    init: mir::Expr::new(
                        site.result.clone(),
                        mir::ExprKind::EnumField {
                            operand: Box::new(mir::Expr::local(step_local, step_ty.clone())),
                            variant: 0,
                            index: 0,
                        },
                    ),
                }));
            }
            statements
        },
        terminator: mir::Terminator::Goto(post),
        unwind,
    });
    let invalid = protocol_error_block(lowerer, module, &mut body.locals, &mut body.blocks, unwind);
    let adapter_claim = body.locals.alloc(local(
        &format!("$completed_adapter_claim.{}", site.state),
        mir::Type::Int,
    ));
    let frame_claim = body.locals.alloc(local(
        &format!("$completed_frame_claim.{}", site.state),
        mir::Type::Int,
    ));
    let claim_frame = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.completed_claim_frame.{}", site.state),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: frame_claim,
            init: atomic_field_compare_exchange(
                mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                0,
                i64::from(site.state),
                STATE_RUNNING,
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(frame_claim, mir::Type::Int),
                i64::from(site.state),
            ),
            then_block: completed,
            else_block: invalid,
        },
        unwind,
    });
    let claim_completed = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.completed_claim_adapter.{}", site.state),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: adapter_claim,
            init: atomic_field_compare_exchange(
                mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                1,
                ADAPTER_WAITING,
                ADAPTER_CONSUMED,
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(adapter_claim, mir::Type::Int),
                ADAPTER_WAITING,
            ),
            then_block: claim_frame,
            else_block: invalid,
        },
        unwind,
    });
    let suspended = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.suspended.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Return {
            value: Some(suspended_value(outer_step)),
        },
        unwind: None,
    });
    body.blocks[site.block].terminator = mir::Terminator::Branch {
        cond: is_completed(step_local, step_ty),
        then_block: claim_completed,
        else_block: suspended,
    };

    let mut resume_statements = Vec::new();
    for local in &site.live_after {
        if Some(*local) == destination {
            continue;
        }
        if let Some(slot) = frame_slots.get(local) {
            resume_statements.push(restore_statement(
                mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                *local,
                slot,
            ));
        }
    }
    if let Some(destination) = destination {
        resume_statements.push(restore_statement(
            mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
            destination,
            &frame_slots[&destination],
        ));
    }
    let resume_block = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.resume.{}", site.state),
        statements: resume_statements,
        terminator: mir::Terminator::Goto(post),
        unwind,
    });
    GeneratedSite {
        state: i64::from(site.state),
        resume_block,
        failure_state: failure_state(site.state),
        failure_block: failure_resume_block(
            body,
            frame_local,
            frame_slots,
            failure_slot,
            &site,
            unwind,
        ),
        point: adapter.point,
    }
}

fn failure_resume_block(
    body: &mut mir::Body,
    frame: mir::LocalId,
    frame_slots: &HashMap<mir::LocalId, FrameSlot>,
    failure_slot: FrameSlot,
    site: &SuspendSite,
    unwind: Option<mir::BlockId>,
) -> mir::BlockId {
    let mut statements = Vec::new();
    for local in &site.live_after {
        if Some(*local) == site.destination {
            continue;
        }
        if let Some(slot) = frame_slots.get(local) {
            statements.push(restore_statement(
                mir::Expr::local(frame, body.locals[frame].ty.clone()),
                *local,
                slot,
            ));
        }
    }
    body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.failure.{}", site.state),
        statements,
        terminator: mir::Terminator::Throw {
            exception: mir::Expr::new(
                failure_slot.value_ty.clone(),
                mir::ExprKind::EnumField {
                    operand: Box::new(frame_field(
                        mir::Expr::local(frame, body.locals[frame].ty.clone()),
                        failure_slot.field,
                        failure_slot.slot_ty.clone(),
                    )),
                    variant: 1,
                    index: 0,
                },
            ),
            unwind,
        },
        unwind,
    })
}

struct GeneratedAdapter {
    class: mir::ClassId,
    point: mir::CoroutineResumePointId,
}

#[allow(clippy::too_many_arguments)]
fn generate_adapter(
    lowerer: &mut Lowerer,
    module: &hir::Module,
    frame_class: mir::ClassId,
    frame: mir::CoroutineFrameId,
    destination: Option<FrameSlot>,
    failure_slot: FrameSlot,
    outer_step: &mir::Type,
    outer_continuation: mir::InterfaceId,
    outer_resume: mir::FunctionId,
    outer_failure: mir::FunctionId,
    source_symbol: &str,
    driver: mir::FunctionId,
    state: u32,
    failure_state: i64,
    result: &mir::Type,
    safe_latches: Option<(FrameSlot, FrameSlot)>,
) -> GeneratedAdapter {
    let protocol = lowerer.coroutine_protocol(module, result);
    let continuation = lowerer.interfaces.mir_id(protocol.continuation);
    let name = format!("CoroutineAdapter${}${state}", sanitize(source_symbol));
    let mut fields = vec![
        mir::Field {
            name: "frame".to_string(),
            ty: mir::Type::Class(frame_class),
        },
        mir::Field {
            name: "status".to_string(),
            ty: mir::Type::Int,
        },
    ];
    if let Some((success, failure)) = safe_latches.as_ref() {
        fields.push(mir::Field {
            name: "result".to_string(),
            ty: success.slot_ty.clone(),
        });
        fields.push(mir::Field {
            name: "failure".to_string(),
            ty: failure.slot_ty.clone(),
        });
    }
    let class = generated_class(lowerer, name, fields, vec![continuation], Vec::new());
    let resume = generate_resume_method(
        lowerer,
        module,
        class,
        frame_class,
        destination,
        outer_step,
        outer_continuation,
        outer_resume,
        outer_failure,
        driver,
        source_symbol,
        state,
        result,
        safe_latches.as_ref().map(|(success, _)| success.clone()),
    );
    let failure = generate_failure_method(
        lowerer,
        module,
        class,
        frame_class,
        failure_slot,
        outer_step,
        outer_continuation,
        outer_resume,
        outer_failure,
        driver,
        source_symbol,
        state,
        failure_state,
        safe_latches.as_ref().map(|(_, failure)| failure.clone()),
    );
    lowerer.classes[class].itables = vec![mir::ItableRecord {
        interface: continuation,
        slots: vec![
            mir::TableSlot::Function(resume),
            mir::TableSlot::Function(failure),
        ],
    }];
    let point = lowerer
        .coroutines
        .resume_points
        .alloc(mir::CoroutineResumePoint {
            frame,
            state,
            result: result.clone(),
            adapter: class,
            resume,
            resume_with_exception: failure,
        });
    GeneratedAdapter { class, point }
}

#[allow(clippy::too_many_arguments)]
fn generate_resume_method(
    lowerer: &mut Lowerer,
    module: &hir::Module,
    adapter: mir::ClassId,
    frame_class: mir::ClassId,
    destination: Option<FrameSlot>,
    outer_step: &mir::Type,
    outer_continuation: mir::InterfaceId,
    outer_resume: mir::FunctionId,
    outer_failure: mir::FunctionId,
    driver: mir::FunctionId,
    source_symbol: &str,
    state: u32,
    result: &mir::Type,
    latch: Option<FrameSlot>,
) -> mir::FunctionId {
    let mut locals = Arena::new();
    let this = locals.alloc(local("this", mir::Type::Class(adapter)));
    let value = locals.alloc(local("value", result.clone()));
    let step = locals.alloc(local("$step", outer_step.clone()));
    let adapter_claim = locals.alloc(local("$adapter_claim", mir::Type::Int));
    let frame_claim = locals.alloc(local("$frame_claim", mir::Type::Int));
    let mut blocks = Arena::new();
    let invalid = protocol_error_block(lowerer, module, &mut locals, &mut blocks, None);
    let exits = drive_exit_blocks(
        lowerer,
        module,
        &mut locals,
        &mut blocks,
        this,
        step,
        adapter,
        frame_class,
        outer_step,
        outer_continuation,
        outer_resume,
        outer_failure,
    );
    let valid = blocks.alloc(mir::BasicBlock {
        name: "valid".to_string(),
        statements: {
            let mut statements = Vec::new();
            if let Some(destination) = destination.as_ref() {
                statements.push(field_set(
                    adapter_frame(this, adapter, frame_class),
                    destination.field,
                    slot_value(destination, mir::Expr::local(value, result.clone())),
                ));
            }
            statements.push(atomic_field_store(
                mir::Expr::local(this, mir::Type::Class(adapter)),
                1,
                mir::Expr::int(ADAPTER_CONSUMED),
            ));
            statements.push(statement(mir::StatementKind::Call(
                mir::CallEffect::Value {
                    destination: step,
                    call: mir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Direct,
                            callee: mir::Callee::User(driver),
                        },
                        args: vec![
                            adapter_frame(this, adapter, frame_class),
                            mir::Expr::int(i64::from(state)),
                        ],
                    },
                },
            )));
            statements
        },
        terminator: mir::Terminator::Branch {
            cond: is_completed(step, outer_step.clone()),
            then_block: exits.completed,
            else_block: exits.suspended,
        },
        unwind: Some(exits.catch_pad),
    });
    let invalid_frame = blocks.alloc(mir::BasicBlock {
        name: "invalid_frame".to_string(),
        statements: vec![atomic_field_store(
            mir::Expr::local(this, mir::Type::Class(adapter)),
            1,
            mir::Expr::int(ADAPTER_CONSUMED),
        )],
        terminator: mir::Terminator::Goto(invalid),
        unwind: None,
    });
    let claim_frame = blocks.alloc(mir::BasicBlock {
        name: "claim_frame".to_string(),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: frame_claim,
            init: atomic_field_compare_exchange(
                adapter_frame(this, adapter, frame_class),
                0,
                i64::from(state),
                STATE_RUNNING,
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(frame_claim, mir::Type::Int),
                i64::from(state),
            ),
            then_block: valid,
            else_block: invalid_frame,
        },
        unwind: None,
    });
    let claim_waiting = blocks.alloc(mir::BasicBlock {
        name: "claim_waiting".to_string(),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: adapter_claim,
            init: atomic_field_compare_exchange(
                mir::Expr::local(this, mir::Type::Class(adapter)),
                1,
                ADAPTER_WAITING,
                ADAPTER_COMPLETING_SUCCESS,
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(adapter_claim, mir::Type::Int),
                ADAPTER_WAITING,
            ),
            then_block: claim_frame,
            else_block: invalid,
        },
        unwind: None,
    });
    let entry = if let Some(latch) = latch.as_ref() {
        let latched = blocks.alloc(mir::BasicBlock {
            name: "latched".to_string(),
            statements: vec![
                field_set(
                    mir::Expr::local(this, mir::Type::Class(adapter)),
                    latch.field,
                    slot_value(latch, mir::Expr::local(value, result.clone())),
                ),
                atomic_field_store(
                    mir::Expr::local(this, mir::Type::Class(adapter)),
                    1,
                    mir::Expr::int(ADAPTER_LATCHED_SUCCESS),
                ),
            ],
            terminator: mir::Terminator::Return { value: None },
            unwind: None,
        });
        blocks.alloc(mir::BasicBlock {
            name: "entry".to_string(),
            statements: vec![statement(mir::StatementKind::ValDecl {
                local: adapter_claim,
                init: atomic_field_compare_exchange(
                    mir::Expr::local(this, mir::Type::Class(adapter)),
                    1,
                    ADAPTER_REGISTERING,
                    ADAPTER_COMPLETING_SUCCESS,
                ),
            })],
            terminator: mir::Terminator::Branch {
                cond: int_eq(
                    mir::Expr::local(adapter_claim, mir::Type::Int),
                    ADAPTER_REGISTERING,
                ),
                then_block: latched,
                else_block: claim_waiting,
            },
            unwind: None,
        })
    } else {
        claim_waiting
    };
    let function = lowerer.functions.alloc(mir::Function {
        gc_effect: mir::GcEffect::Managed,
        name: format!("CoroutineAdapter.resume${state}"),
        symbol: format!("{source_symbol}$resume${state}"),
        params: vec![
            mir::Param {
                name: "this".to_string(),
                ty: mir::Type::Class(adapter),
                local: this,
            },
            mir::Param {
                name: "value".to_string(),
                ty: result.clone(),
                local: value,
            },
        ],
        return_ty: mir::Type::Unit,
        body: mir::Body {
            locals,
            blocks,
            entry,
        },
    });
    lowerer.top_level.push(function);
    function
}

#[allow(clippy::too_many_arguments)]
fn generate_failure_method(
    lowerer: &mut Lowerer,
    module: &hir::Module,
    adapter: mir::ClassId,
    frame_class: mir::ClassId,
    failure_slot: FrameSlot,
    outer_step: &mir::Type,
    outer_continuation: mir::InterfaceId,
    outer_resume: mir::FunctionId,
    outer_failure: mir::FunctionId,
    driver: mir::FunctionId,
    source_symbol: &str,
    state: u32,
    failure_state: i64,
    latch: Option<FrameSlot>,
) -> mir::FunctionId {
    let throwable = mir::Type::Class(lowerer.class_map[&module.exception_core.throwable.class()]);
    let mut locals = Arena::new();
    let this = locals.alloc(local("this", mir::Type::Class(adapter)));
    let exception = locals.alloc(local("exception", throwable.clone()));
    let step = locals.alloc(local("$step", outer_step.clone()));
    let adapter_claim = locals.alloc(local("$adapter_claim", mir::Type::Int));
    let frame_claim = locals.alloc(local("$frame_claim", mir::Type::Int));
    let mut blocks = Arena::new();
    let invalid = protocol_error_block(lowerer, module, &mut locals, &mut blocks, None);
    let exits = drive_exit_blocks(
        lowerer,
        module,
        &mut locals,
        &mut blocks,
        this,
        step,
        adapter,
        frame_class,
        outer_step,
        outer_continuation,
        outer_resume,
        outer_failure,
    );
    let valid = blocks.alloc(mir::BasicBlock {
        name: "valid".to_string(),
        statements: vec![
            field_set(
                adapter_frame(this, adapter, frame_class),
                failure_slot.field,
                slot_value(
                    &failure_slot,
                    mir::Expr::local(exception, throwable.clone()),
                ),
            ),
            atomic_field_store(
                mir::Expr::local(this, mir::Type::Class(adapter)),
                1,
                mir::Expr::int(ADAPTER_CONSUMED),
            ),
            statement(mir::StatementKind::Call(mir::CallEffect::Value {
                destination: step,
                call: mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(driver),
                    },
                    args: vec![
                        adapter_frame(this, adapter, frame_class),
                        mir::Expr::int(failure_state),
                    ],
                },
            })),
        ],
        terminator: mir::Terminator::Branch {
            cond: is_completed(step, outer_step.clone()),
            then_block: exits.completed,
            else_block: exits.suspended,
        },
        unwind: Some(exits.catch_pad),
    });
    let invalid_frame = blocks.alloc(mir::BasicBlock {
        name: "invalid_frame".to_string(),
        statements: vec![atomic_field_store(
            mir::Expr::local(this, mir::Type::Class(adapter)),
            1,
            mir::Expr::int(ADAPTER_CONSUMED),
        )],
        terminator: mir::Terminator::Goto(invalid),
        unwind: None,
    });
    let claim_frame = blocks.alloc(mir::BasicBlock {
        name: "claim_frame".to_string(),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: frame_claim,
            init: atomic_field_compare_exchange(
                adapter_frame(this, adapter, frame_class),
                0,
                i64::from(state),
                STATE_RUNNING,
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(frame_claim, mir::Type::Int),
                i64::from(state),
            ),
            then_block: valid,
            else_block: invalid_frame,
        },
        unwind: None,
    });
    let claim_waiting = blocks.alloc(mir::BasicBlock {
        name: "claim_waiting".to_string(),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: adapter_claim,
            init: atomic_field_compare_exchange(
                mir::Expr::local(this, mir::Type::Class(adapter)),
                1,
                ADAPTER_WAITING,
                ADAPTER_COMPLETING_FAILURE,
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(adapter_claim, mir::Type::Int),
                ADAPTER_WAITING,
            ),
            then_block: claim_frame,
            else_block: invalid,
        },
        unwind: None,
    });
    let entry = if let Some(latch) = latch.as_ref() {
        let latched = blocks.alloc(mir::BasicBlock {
            name: "latched".to_string(),
            statements: vec![
                field_set(
                    mir::Expr::local(this, mir::Type::Class(adapter)),
                    latch.field,
                    slot_value(latch, mir::Expr::local(exception, throwable.clone())),
                ),
                atomic_field_store(
                    mir::Expr::local(this, mir::Type::Class(adapter)),
                    1,
                    mir::Expr::int(ADAPTER_LATCHED_FAILURE),
                ),
            ],
            terminator: mir::Terminator::Return { value: None },
            unwind: None,
        });
        blocks.alloc(mir::BasicBlock {
            name: "entry".to_string(),
            statements: vec![statement(mir::StatementKind::ValDecl {
                local: adapter_claim,
                init: atomic_field_compare_exchange(
                    mir::Expr::local(this, mir::Type::Class(adapter)),
                    1,
                    ADAPTER_REGISTERING,
                    ADAPTER_COMPLETING_FAILURE,
                ),
            })],
            terminator: mir::Terminator::Branch {
                cond: int_eq(
                    mir::Expr::local(adapter_claim, mir::Type::Int),
                    ADAPTER_REGISTERING,
                ),
                then_block: latched,
                else_block: claim_waiting,
            },
            unwind: None,
        })
    } else {
        claim_waiting
    };
    let function = lowerer.functions.alloc(mir::Function {
        gc_effect: mir::GcEffect::Managed,
        name: format!("CoroutineAdapter.resumeWithException${state}"),
        symbol: format!("{source_symbol}$resume_exception${state}"),
        params: vec![
            mir::Param {
                name: "this".to_string(),
                ty: mir::Type::Class(adapter),
                local: this,
            },
            mir::Param {
                name: "exception".to_string(),
                ty: throwable,
                local: exception,
            },
        ],
        return_ty: mir::Type::Unit,
        body: mir::Body {
            locals,
            blocks,
            entry,
        },
    });
    lowerer.top_level.push(function);
    function
}

struct DriveExitBlocks {
    completed: mir::BlockId,
    suspended: mir::BlockId,
    catch_pad: mir::BlockId,
}

#[allow(clippy::too_many_arguments)]
fn drive_exit_blocks(
    lowerer: &Lowerer,
    module: &hir::Module,
    locals: &mut Arena<mir::Local>,
    blocks: &mut Arena<mir::BasicBlock>,
    this: mir::LocalId,
    step: mir::LocalId,
    adapter: mir::ClassId,
    frame_class: mir::ClassId,
    step_ty: &mir::Type,
    outer_continuation: mir::InterfaceId,
    outer_resume: mir::FunctionId,
    outer_failure: mir::FunctionId,
) -> DriveExitBlocks {
    let throwable = mir::Type::Class(lowerer.class_map[&module.exception_core.throwable.class()]);
    let exception = locals.alloc(local("$uncaught", throwable.clone()));
    let completion_ty = mir::Type::Interface(outer_continuation);
    let completed_value_ty = lowerer.functions[outer_resume].params[1].ty.clone();
    let completed = blocks.alloc(mir::BasicBlock {
        name: "completed".to_string(),
        statements: vec![
            atomic_field_store(
                adapter_frame(this, adapter, frame_class),
                0,
                mir::Expr::int(STATE_COMPLETED),
            ),
            statement(mir::StatementKind::Call(mir::CallEffect::Unit(mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Interface {
                        interface: outer_continuation,
                        slot: 0,
                    },
                    callee: mir::Callee::User(outer_resume),
                },
                args: vec![
                    frame_field(
                        adapter_frame(this, adapter, frame_class),
                        1,
                        completion_ty.clone(),
                    ),
                    mir::Expr::new(
                        completed_value_ty,
                        mir::ExprKind::EnumField {
                            operand: Box::new(mir::Expr::local(step, step_ty.clone())),
                            variant: 0,
                            index: 0,
                        },
                    ),
                ],
            }))),
        ],
        terminator: mir::Terminator::Return { value: None },
        unwind: None,
    });
    let suspended = blocks.alloc(mir::BasicBlock {
        name: "suspended".to_string(),
        statements: Vec::new(),
        terminator: mir::Terminator::Return { value: None },
        unwind: None,
    });
    let failed = blocks.alloc(mir::BasicBlock {
        name: "failed".to_string(),
        statements: vec![
            atomic_field_store(
                adapter_frame(this, adapter, frame_class),
                0,
                mir::Expr::int(STATE_COMPLETED),
            ),
            statement(mir::StatementKind::Call(mir::CallEffect::Unit(mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Interface {
                        interface: outer_continuation,
                        slot: 1,
                    },
                    callee: mir::Callee::User(outer_failure),
                },
                args: vec![
                    frame_field(adapter_frame(this, adapter, frame_class), 1, completion_ty),
                    mir::Expr::local(exception, throwable.clone()),
                ],
            }))),
        ],
        terminator: mir::Terminator::Return { value: None },
        unwind: None,
    });
    let catch_pad = blocks.alloc(mir::BasicBlock {
        name: "body_failure".to_string(),
        statements: vec![
            statement(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                cleanup: false,
            })),
            statement(mir::StatementKind::Eh(mir::EhStatement::BeginCatch)),
            statement(mir::StatementKind::Call(mir::CallEffect::Value {
                destination: exception,
                call: mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::Runtime(mir::RuntimeFn::MaterializeException),
                    },
                    args: vec![mir::Expr::caught_exception()],
                },
            })),
            statement(mir::StatementKind::Eh(mir::EhStatement::EndCatch)),
        ],
        terminator: mir::Terminator::Goto(failed),
        unwind: None,
    });
    DriveExitBlocks {
        completed,
        suspended,
        catch_pad,
    }
}

fn generated_class(
    lowerer: &mut Lowerer,
    name: String,
    fields: Vec<mir::Field>,
    interfaces: Vec<mir::InterfaceId>,
    itables: Vec<mir::ItableRecord>,
) -> mir::ClassId {
    let class = lowerer.classes.alloc(mir::ClassDef {
        modifier: mir::ClassModifier::Final,
        name: name.clone(),
        representation: mir::ClassRepresentation::Declared {
            fields,
            base_class: None,
        },
        interfaces,
        vtable: Vec::new(),
        itables,
    });
    let shell = lowerer.shell.classes.alloc(mir::ClassDef {
        modifier: mir::ClassModifier::Final,
        name,
        representation: mir::ClassRepresentation::Declared {
            fields: Vec::new(),
            base_class: None,
        },
        interfaces: Vec::new(),
        vtable: Vec::new(),
        itables: Vec::new(),
    });
    assert_eq!(class, shell, "the mangling shell mirrors class ids");
    class
}

fn wrapper_params(
    old: &[mir::Param],
) -> (
    Vec<mir::Param>,
    Arena<mir::Local>,
    HashMap<mir::LocalId, mir::LocalId>,
    mir::LocalId,
) {
    let mut locals = Arena::new();
    let mut params = Vec::new();
    let mut map = HashMap::new();
    for param in old {
        let local = locals.alloc(local(&param.name, param.ty.clone()));
        params.push(mir::Param {
            name: param.name.clone(),
            ty: param.ty.clone(),
            local,
        });
        map.insert(param.local, local);
    }
    let completion = params
        .last()
        .expect("hidden completion parameter is retained")
        .local;
    (params, locals, map, completion)
}

#[allow(clippy::too_many_arguments)]
fn wrapper_body(
    frame_class: mir::ClassId,
    mut locals: Arena<mir::Local>,
    saved: &[mir::LocalId],
    frame_slots: &HashMap<mir::LocalId, FrameSlot>,
    params: &HashMap<mir::LocalId, mir::LocalId>,
    completion: mir::LocalId,
    failure_slot: FrameSlot,
    driver: mir::FunctionId,
    step_ty: &mir::Type,
) -> mir::Body {
    let frame = locals.alloc(local("$frame", mir::Type::Class(frame_class)));
    let step = locals.alloc(local("$step", step_ty.clone()));
    let mut args = vec![
        mir::Expr::int(STATE_INITIAL),
        mir::Expr::local(completion, locals[completion].ty.clone()),
    ];
    for old_local in saved {
        let slot = &frame_slots[old_local];
        args.push(match params.get(old_local) {
            Some(local) => slot_value(slot, mir::Expr::local(*local, locals[*local].ty.clone())),
            None => slot_empty(slot),
        });
    }
    args.push(slot_empty(&failure_slot));
    let mut blocks = Arena::new();
    let entry = blocks.alloc(mir::BasicBlock {
        name: "entry".to_string(),
        statements: vec![
            statement(mir::StatementKind::ValDecl {
                local: frame,
                init: mir::Expr::new(
                    mir::Type::Class(frame_class),
                    mir::ExprKind::ClassInit {
                        class_id: frame_class,
                        args,
                    },
                ),
            }),
            statement(mir::StatementKind::Call(mir::CallEffect::Value {
                destination: step,
                call: mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(driver),
                    },
                    args: vec![
                        mir::Expr::local(frame, mir::Type::Class(frame_class)),
                        mir::Expr::int(STATE_INITIAL),
                    ],
                },
            })),
        ],
        terminator: mir::Terminator::Return {
            value: Some(mir::Expr::local(step, step_ty.clone())),
        },
        unwind: None,
    });
    mir::Body {
        locals,
        blocks,
        entry,
    }
}

fn dispatch_block(
    blocks: &mut Arena<mir::BasicBlock>,
    dispatch_state: mir::LocalId,
    state: i64,
    target: mir::BlockId,
    otherwise: mir::BlockId,
) -> mir::BlockId {
    blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.dispatch.{state}"),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: int_eq(mir::Expr::local(dispatch_state, mir::Type::Int), state),
            then_block: target,
            else_block: otherwise,
        },
        unwind: None,
    })
}

fn protocol_error_block(
    lowerer: &Lowerer,
    module: &hir::Module,
    locals: &mut Arena<mir::Local>,
    blocks: &mut Arena<mir::BasicBlock>,
    unwind: Option<mir::BlockId>,
) -> mir::BlockId {
    let class = module.exception_core.illegal_state_exception.class();
    let mir_class = lowerer.class_map[&class];
    let exception = locals.alloc(local("$protocol_error", mir::Type::Class(mir_class)));
    blocks.alloc(mir::BasicBlock {
        name: "coroutine.protocol_error".to_string(),
        statements: vec![statement(mir::StatementKind::Call(
            mir::CallEffect::Value {
                destination: exception,
                call: mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(lowerer.ctors[&class]),
                    },
                    args: Vec::new(),
                },
            },
        ))],
        terminator: mir::Terminator::Throw {
            exception: mir::Expr::local(exception, mir::Type::Class(mir_class)),
            unwind,
        },
        unwind,
    })
}

fn save_statement(frame: mir::Expr, value: mir::Expr, slot: &FrameSlot) -> mir::Statement {
    field_set(frame, slot.field, slot_value(slot, value))
}

fn restore_statement(frame: mir::Expr, local: mir::LocalId, slot: &FrameSlot) -> mir::Statement {
    statement(mir::StatementKind::Assign {
        local,
        value: mir::Expr::new(
            slot.value_ty.clone(),
            mir::ExprKind::EnumField {
                operand: Box::new(frame_field(frame, slot.field, slot.slot_ty.clone())),
                variant: 1,
                index: 0,
            },
        ),
    })
}

fn slot_empty(slot: &FrameSlot) -> mir::Expr {
    mir::Expr::new(
        slot.slot_ty.clone(),
        mir::ExprKind::VariantConstruct {
            variant: 0,
            fields: Vec::new(),
        },
    )
}

fn slot_value(slot: &FrameSlot, value: mir::Expr) -> mir::Expr {
    debug_assert_eq!(value.ty, slot.value_ty);
    mir::Expr::new(
        slot.slot_ty.clone(),
        mir::ExprKind::VariantConstruct {
            variant: 1,
            fields: vec![value],
        },
    )
}

fn suspended_value(step: &mir::Type) -> mir::Expr {
    mir::Expr::new(
        step.clone(),
        mir::ExprKind::VariantConstruct {
            variant: 1,
            fields: Vec::new(),
        },
    )
}

fn is_completed(step: mir::LocalId, step_ty: mir::Type) -> mir::Expr {
    int_eq(mir::Expr::enum_tag(mir::Expr::local(step, step_ty)), 0)
}

fn int_eq(lhs: mir::Expr, rhs: i64) -> mir::Expr {
    mir::Expr::new(
        mir::Type::Boolean,
        mir::ExprKind::Binary {
            op: mir::BinOp::IntEq,
            lhs: Box::new(lhs),
            rhs: Box::new(mir::Expr::int(rhs)),
        },
    )
}

fn adapter_frame(this: mir::LocalId, adapter: mir::ClassId, frame: mir::ClassId) -> mir::Expr {
    frame_field(
        mir::Expr::local(this, mir::Type::Class(adapter)),
        0,
        mir::Type::Class(frame),
    )
}

fn frame_field(frame: mir::Expr, index: u32, ty: mir::Type) -> mir::Expr {
    mir::Expr::new(
        ty,
        mir::ExprKind::FieldAccess {
            receiver: Box::new(frame),
            index,
        },
    )
}

fn field_set(object: mir::Expr, index: u32, value: mir::Expr) -> mir::Statement {
    statement(mir::StatementKind::FieldSet {
        object,
        index,
        value,
    })
}

fn atomic_field_load(object: mir::Expr, index: u32) -> mir::Expr {
    mir::Expr::new(
        mir::Type::Int,
        mir::ExprKind::AtomicFieldLoad {
            object: Box::new(object),
            index,
        },
    )
}

fn atomic_field_store(object: mir::Expr, index: u32, value: mir::Expr) -> mir::Statement {
    statement(mir::StatementKind::AtomicFieldStore {
        object,
        index,
        value,
    })
}

fn atomic_field_compare_exchange(
    object: mir::Expr,
    index: u32,
    expected: i64,
    replacement: i64,
) -> mir::Expr {
    mir::Expr::new(
        mir::Type::Int,
        mir::ExprKind::AtomicFieldCompareExchange {
            object: Box::new(object),
            index,
            expected: Box::new(mir::Expr::int(expected)),
            replacement: Box::new(mir::Expr::int(replacement)),
        },
    )
}

fn statement(kind: mir::StatementKind) -> mir::Statement {
    mir::Statement {
        kind,
        span: Span { start: 0, end: 0 },
    }
}

fn local(name: &str, ty: mir::Type) -> mir::Local {
    mir::Local {
        name: name.to_string(),
        ty,
        mutable: false,
    }
}

fn callee_return_type(lowerer: &Lowerer, callee: mir::Callee) -> mir::Type {
    let function = match callee {
        mir::Callee::User(function) => function,
        mir::Callee::Monomorphized(instance) => lowerer.instances.meta[instance].function,
        mir::Callee::Extern(extern_id) => {
            return lowerer.extern_functions[extern_id].return_type.clone();
        }
        mir::Callee::Closure(function_type) | mir::Callee::FunctionBridge(function_type) => {
            let signature = &lowerer.shell.function_types[function_type];
            return if signature.is_suspend {
                lowerer
                    .coroutines
                    .step_type_for(&signature.return_type)
                    .expect("every suspend function result has a CoroutineStep type")
            } else {
                signature.return_type.clone()
            };
        }
        mir::Callee::CoroutineSuspend { .. } | mir::Callee::Runtime(_) => {
            unreachable!("a source suspend call resolves to a hidden-ABI function")
        }
    };
    lowerer.functions[function].return_ty.clone()
}

fn sanitize(symbol: &str) -> String {
    symbol
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn rewrite_intrinsic_site(
    lowerer: &mut Lowerer,
    module: &hir::Module,
    body: &mut mir::Body,
    frame_local: mir::LocalId,
    frame_class: mir::ClassId,
    frame: mir::CoroutineFrameId,
    frame_slots: &HashMap<mir::LocalId, FrameSlot>,
    failure_slot: FrameSlot,
    outer_step: &mir::Type,
    outer_continuation: mir::InterfaceId,
    outer_resume: mir::FunctionId,
    outer_failure: mir::FunctionId,
    source_symbol: &str,
    driver: mir::FunctionId,
    site: SuspendSite,
    mut call: mir::Call,
    post: mir::BlockId,
    unwind: Option<mir::BlockId>,
) -> GeneratedSite {
    let SuspendKind::Intrinsic { register } = site.kind else {
        unreachable!("intrinsic site carries its concrete register method")
    };
    let throwable = mir::Type::Class(lowerer.class_map[&module.exception_core.throwable.class()]);
    let (_, result_latch_ty) = lowerer.coroutines.slot_for(
        &site.result,
        &lowerer.structs,
        &mut lowerer.enums,
        &mut lowerer.shell,
    );
    let (_, failure_latch_ty) = lowerer.coroutines.slot_for(
        &throwable,
        &lowerer.structs,
        &mut lowerer.enums,
        &mut lowerer.shell,
    );
    let result_latch = FrameSlot {
        field: 2,
        slot_ty: result_latch_ty,
        value_ty: site.result.clone(),
    };
    let failure_latch = FrameSlot {
        field: 3,
        slot_ty: failure_latch_ty,
        value_ty: throwable.clone(),
    };
    let adapter = generate_adapter(
        lowerer,
        module,
        frame_class,
        frame,
        site.destination.map(|local| frame_slots[&local].clone()),
        failure_slot.clone(),
        outer_step,
        outer_continuation,
        outer_resume,
        outer_failure,
        source_symbol,
        driver,
        site.state,
        failure_state(site.state),
        &site.result,
        Some((result_latch.clone(), failure_latch.clone())),
    );
    let adapter_local = body.locals.alloc(local(
        &format!("$safe_adapter.{}", site.state),
        mir::Type::Class(adapter.class),
    ));
    let exception = body.locals.alloc(local(
        &format!("$resume_exception.{}", site.state),
        throwable.clone(),
    ));
    let registration_claim = body.locals.alloc(local(
        &format!("$registration_claim.{}", site.state),
        mir::Type::Int,
    ));
    let frame_claim = body.locals.alloc(local(
        &format!("$registration_frame_claim.{}", site.state),
        mir::Type::Int,
    ));

    let current = &mut body.blocks[site.block];
    for local in site
        .live_after
        .iter()
        .copied()
        .filter(|local| Some(*local) != site.destination)
    {
        if let Some(slot) = frame_slots.get(&local) {
            current.statements.push(save_statement(
                mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                mir::Expr::local(local, body.locals[local].ty.clone()),
                slot,
            ));
        }
    }
    current.statements.push(atomic_field_store(
        mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
        0,
        mir::Expr::int(i64::from(site.state)),
    ));
    current
        .statements
        .push(statement(mir::StatementKind::ValDecl {
            local: adapter_local,
            init: mir::Expr::new(
                mir::Type::Class(adapter.class),
                mir::ExprKind::ClassInit {
                    class_id: adapter.class,
                    args: vec![
                        mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                        mir::Expr::int(ADAPTER_REGISTERING),
                        slot_empty(&result_latch),
                        slot_empty(&failure_latch),
                    ],
                },
            ),
        }));
    call.target.callee = mir::Callee::Monomorphized(register);
    call.args.push(mir::Expr::local(
        adapter_local,
        mir::Type::Class(adapter.class),
    ));
    let register_call = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.register.{}", site.state),
        statements: vec![statement(mir::StatementKind::Call(mir::CallEffect::Unit(
            call,
        )))],
        terminator: mir::Terminator::Unreachable,
        unwind: None,
    });
    body.blocks[site.block].terminator = mir::Terminator::Goto(register_call);

    let success = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registered_success.{}", site.state),
        statements: {
            let mut statements = vec![atomic_field_store(
                mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                1,
                mir::Expr::int(ADAPTER_CONSUMED),
            )];
            if let Some(destination) = site.destination {
                statements.push(statement(mir::StatementKind::ValDecl {
                    local: destination,
                    init: mir::Expr::new(
                        site.result.clone(),
                        mir::ExprKind::EnumField {
                            operand: Box::new(frame_field(
                                mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                                result_latch.field,
                                result_latch.slot_ty.clone(),
                            )),
                            variant: 1,
                            index: 0,
                        },
                    ),
                }));
            }
            statements
        },
        terminator: mir::Terminator::Goto(post),
        unwind,
    });
    let failure = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registered_failure.{}", site.state),
        statements: vec![
            atomic_field_store(
                mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                1,
                mir::Expr::int(ADAPTER_CONSUMED),
            ),
            statement(mir::StatementKind::ValDecl {
                local: exception,
                init: mir::Expr::new(
                    throwable.clone(),
                    mir::ExprKind::EnumField {
                        operand: Box::new(frame_field(
                            mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                            failure_latch.field,
                            failure_latch.slot_ty.clone(),
                        )),
                        variant: 1,
                        index: 0,
                    },
                ),
            }),
        ],
        terminator: mir::Terminator::Throw {
            exception: mir::Expr::local(exception, throwable.clone()),
            unwind,
        },
        unwind,
    });
    let suspended = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registered_suspend.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Return {
            value: Some(suspended_value(outer_step)),
        },
        unwind: None,
    });
    let invalid = protocol_error_block(lowerer, module, &mut body.locals, &mut body.blocks, unwind);
    let claim_success_frame = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.claim_success_frame.{}", site.state),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: frame_claim,
            init: atomic_field_compare_exchange(
                mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                0,
                i64::from(site.state),
                STATE_RUNNING,
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(frame_claim, mir::Type::Int),
                i64::from(site.state),
            ),
            then_block: success,
            else_block: invalid,
        },
        unwind,
    });
    let claim_failure_frame = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.claim_failure_frame.{}", site.state),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: frame_claim,
            init: atomic_field_compare_exchange(
                mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                0,
                i64::from(site.state),
                STATE_RUNNING,
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(frame_claim, mir::Type::Int),
                i64::from(site.state),
            ),
            then_block: failure,
            else_block: invalid,
        },
        unwind,
    });
    let check_latched_failure = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.check_latched_failure.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                atomic_field_load(
                    mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                    1,
                ),
                ADAPTER_LATCHED_FAILURE,
            ),
            then_block: claim_failure_frame,
            else_block: invalid,
        },
        unwind,
    });
    let check_latched_success = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.check_latched_success.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                atomic_field_load(
                    mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                    1,
                ),
                ADAPTER_LATCHED_SUCCESS,
            ),
            then_block: claim_success_frame,
            else_block: check_latched_failure,
        },
        unwind,
    });
    let completion_spin = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.completion_backoff.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Unreachable,
        unwind: None,
    });
    let check_completing_failure = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.check_completing_failure.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                atomic_field_load(
                    mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                    1,
                ),
                ADAPTER_COMPLETING_FAILURE,
            ),
            then_block: completion_spin,
            else_block: check_latched_success,
        },
        unwind,
    });
    let completion_wait = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.completion_wait.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                atomic_field_load(
                    mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                    1,
                ),
                ADAPTER_COMPLETING_SUCCESS,
            ),
            then_block: completion_spin,
            else_block: check_completing_failure,
        },
        unwind,
    });
    body.blocks[completion_spin].terminator = mir::Terminator::Goto(completion_wait);
    let register_return = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.register_return.{}", site.state),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: registration_claim,
            init: atomic_field_compare_exchange(
                mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                1,
                ADAPTER_REGISTERING,
                ADAPTER_WAITING,
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(registration_claim, mir::Type::Int),
                ADAPTER_REGISTERING,
            ),
            then_block: suspended,
            else_block: completion_wait,
        },
        unwind,
    });
    let registration_propagate = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registration_propagate.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Throw {
            exception: mir::Expr::local(exception, throwable.clone()),
            unwind,
        },
        unwind,
    });
    let registration_claim_frame = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registration_claim_frame.{}", site.state),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: frame_claim,
            init: atomic_field_compare_exchange(
                mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                0,
                i64::from(site.state),
                STATE_RUNNING,
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(frame_claim, mir::Type::Int),
                i64::from(site.state),
            ),
            then_block: registration_propagate,
            else_block: invalid,
        },
        unwind,
    });
    let registration_protocol = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registration_protocol.{}", site.state),
        statements: vec![atomic_field_store(
            mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
            1,
            mir::Expr::int(ADAPTER_CONSUMED),
        )],
        terminator: mir::Terminator::Goto(invalid),
        unwind,
    });
    let registration_protocol_claim_frame = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registration_protocol_claim_frame.{}", site.state),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: frame_claim,
            init: atomic_field_compare_exchange(
                mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                0,
                i64::from(site.state),
                STATE_RUNNING,
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(frame_claim, mir::Type::Int),
                i64::from(site.state),
            ),
            then_block: registration_protocol,
            else_block: invalid,
        },
        unwind,
    });
    let registration_spin = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registration_backoff.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Unreachable,
        unwind: None,
    });
    let registration_check_completing_failure = body.blocks.alloc(mir::BasicBlock {
        name: format!(
            "coroutine.registration_check_completing_failure.{}",
            site.state
        ),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                atomic_field_load(
                    mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                    1,
                ),
                ADAPTER_COMPLETING_FAILURE,
            ),
            then_block: registration_spin,
            else_block: registration_protocol_claim_frame,
        },
        unwind,
    });
    let registration_wait = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registration_wait.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                atomic_field_load(
                    mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                    1,
                ),
                ADAPTER_COMPLETING_SUCCESS,
            ),
            then_block: registration_spin,
            else_block: registration_check_completing_failure,
        },
        unwind,
    });
    body.blocks[registration_spin].terminator = mir::Terminator::Goto(registration_wait);
    let registration_failure = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registration_failure.{}", site.state),
        statements: vec![
            statement(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                cleanup: false,
            })),
            statement(mir::StatementKind::Eh(mir::EhStatement::BeginCatch)),
            statement(mir::StatementKind::Call(mir::CallEffect::Value {
                destination: exception,
                call: mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::Runtime(mir::RuntimeFn::MaterializeException),
                    },
                    args: vec![mir::Expr::caught_exception()],
                },
            })),
            statement(mir::StatementKind::Eh(mir::EhStatement::EndCatch)),
            statement(mir::StatementKind::ValDecl {
                local: registration_claim,
                init: atomic_field_compare_exchange(
                    mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                    1,
                    ADAPTER_REGISTERING,
                    ADAPTER_CONSUMED,
                ),
            }),
        ],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(registration_claim, mir::Type::Int),
                ADAPTER_REGISTERING,
            ),
            then_block: registration_claim_frame,
            else_block: registration_wait,
        },
        unwind: None,
    });
    body.blocks[register_call].terminator = mir::Terminator::Goto(register_return);
    body.blocks[register_call].unwind = Some(registration_failure);

    let mut resume_statements = Vec::new();
    for local in &site.live_after {
        if Some(*local) == site.destination {
            continue;
        }
        if let Some(slot) = frame_slots.get(local) {
            resume_statements.push(restore_statement(
                mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                *local,
                slot,
            ));
        }
    }
    if let Some(destination) = site.destination {
        resume_statements.push(restore_statement(
            mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
            destination,
            &frame_slots[&destination],
        ));
    }
    let resume_block = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.resume.{}", site.state),
        statements: resume_statements,
        terminator: mir::Terminator::Goto(post),
        unwind,
    });
    GeneratedSite {
        state: i64::from(site.state),
        resume_block,
        failure_state: failure_state(site.state),
        failure_block: failure_resume_block(
            body,
            frame_local,
            frame_slots,
            failure_slot,
            &site,
            unwind,
        ),
        point: adapter.point,
    }
}

fn analyze_sites(lowerer: &Lowerer, body: &mir::Body) -> Vec<SuspendSite> {
    let block_count = body.blocks.len();
    let mut uses = vec![HashSet::new(); block_count];
    let mut defs = vec![HashSet::new(); block_count];
    for (block_id, block) in body.blocks.iter() {
        let index = raw(block_id);
        for statement in &block.statements {
            let (statement_uses, statement_defs) = statement_use_def(statement);
            for local in statement_uses {
                if !defs[index].contains(&local) {
                    uses[index].insert(local);
                }
            }
            defs[index].extend(statement_defs);
        }
        for local in terminator_uses(&block.terminator) {
            if !defs[index].contains(&local) {
                uses[index].insert(local);
            }
        }
    }

    let mut live_in = vec![HashSet::new(); block_count];
    let mut live_out = vec![HashSet::new(); block_count];
    loop {
        let mut changed = false;
        for (block_id, block) in body.blocks.iter().rev() {
            let index = raw(block_id);
            let mut out = HashSet::new();
            for successor in successors(block) {
                out.extend(live_in[raw(successor)].iter().copied());
            }
            let mut input = uses[index].clone();
            input.extend(out.iter().filter(|local| !defs[index].contains(local)));
            if out != live_out[index] || input != live_in[index] {
                live_out[index] = out;
                live_in[index] = input;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut sites = Vec::new();
    for (block_id, block) in body.blocks.iter() {
        let mut live = live_out[raw(block_id)].clone();
        live.extend(terminator_uses(&block.terminator));
        for (statement_index, statement) in block.statements.iter().enumerate().rev() {
            if let Some((destination, result, kind)) = suspend_effect(lowerer, body, statement) {
                let mut live_after: Vec<_> = live.iter().copied().collect();
                live_after.sort_by_key(|local| raw(*local));
                sites.push(SuspendSite {
                    block: block_id,
                    statement: statement_index,
                    destination,
                    result,
                    kind,
                    live_after,
                    state: 0,
                });
            }
            let (statement_uses, statement_defs) = statement_use_def(statement);
            for local in statement_defs {
                live.remove(&local);
            }
            live.extend(statement_uses);
        }
    }
    sites.sort_by_key(|site| (raw(site.block), site.statement));
    sites
}

fn suspend_effect(
    lowerer: &Lowerer,
    body: &mir::Body,
    statement: &mir::Statement,
) -> Option<(Option<mir::LocalId>, mir::Type, SuspendKind)> {
    let mir::StatementKind::Call(effect) = &statement.kind else {
        return None;
    };
    let (call, destination) = match effect {
        mir::CallEffect::Unit(call) => (call, None),
        mir::CallEffect::Value { destination, call } => (call, Some(*destination)),
    };
    if let mir::Callee::CoroutineSuspend { register } = call.target.callee {
        let result = destination
            .map(|local| body.locals[local].ty.clone())
            .unwrap_or(mir::Type::Unit);
        return Some((destination, result, SuspendKind::Intrinsic { register }));
    }
    let function = match call.target.callee {
        mir::Callee::User(function) => function,
        mir::Callee::Monomorphized(instance) => lowerer.instances.meta[instance].function,
        mir::Callee::Closure(function_type) | mir::Callee::FunctionBridge(function_type) => {
            let signature = &lowerer.shell.function_types[function_type];
            return signature.is_suspend.then(|| {
                (
                    destination,
                    signature.return_type.clone(),
                    SuspendKind::Call,
                )
            });
        }
        mir::Callee::CoroutineSuspend { .. } | mir::Callee::Extern(_) | mir::Callee::Runtime(_) => {
            return None;
        }
    };
    lowerer
        .coroutines
        .functions
        .iter()
        .find_map(|(_, coroutine)| {
            (coroutine.function == function).then(|| {
                (
                    destination,
                    coroutine.source_return.clone(),
                    SuspendKind::Call,
                )
            })
        })
}

fn statement_use_def(statement: &mir::Statement) -> (HashSet<mir::LocalId>, HashSet<mir::LocalId>) {
    let mut uses = HashSet::new();
    let mut defs = HashSet::new();
    match &statement.kind {
        mir::StatementKind::Expr(expr) => expr_uses(expr, &mut uses),
        mir::StatementKind::Call(effect) => match effect {
            mir::CallEffect::Unit(call) => call_uses(call, &mut uses),
            mir::CallEffect::Value { destination, call } => {
                call_uses(call, &mut uses);
                defs.insert(*destination);
            }
        },
        mir::StatementKind::ValDecl { local, init } => {
            expr_uses(init, &mut uses);
            defs.insert(*local);
        }
        mir::StatementKind::Assign { local, value } => {
            expr_uses(value, &mut uses);
            defs.insert(*local);
        }
        mir::StatementKind::GlobalAssign { value, .. } => {
            expr_uses(value, &mut uses);
        }
        mir::StatementKind::ArraySet {
            array,
            index,
            value,
            ..
        } => {
            expr_uses(array, &mut uses);
            expr_uses(index, &mut uses);
            expr_uses(value, &mut uses);
        }
        mir::StatementKind::FieldSet { object, value, .. }
        | mir::StatementKind::AtomicFieldStore { object, value, .. } => {
            expr_uses(object, &mut uses);
            expr_uses(value, &mut uses);
        }
        mir::StatementKind::Eh(_) => {}
    }
    (uses, defs)
}

fn terminator_uses(terminator: &mir::Terminator) -> HashSet<mir::LocalId> {
    let mut uses = HashSet::new();
    match terminator {
        mir::Terminator::Branch { cond, .. } => expr_uses(cond, &mut uses),
        mir::Terminator::Return { value } => {
            if let Some(value) = value {
                expr_uses(value, &mut uses);
            }
        }
        mir::Terminator::Throw { exception, .. } => expr_uses(exception, &mut uses),
        mir::Terminator::Goto(_)
        | mir::Terminator::Rethrow { .. }
        | mir::Terminator::Resume
        | mir::Terminator::Trap { .. }
        | mir::Terminator::Unreachable => {}
    }
    uses
}

fn successors(block: &mir::BasicBlock) -> Vec<mir::BlockId> {
    let mut out = Vec::new();
    match block.terminator {
        mir::Terminator::Goto(target) => out.push(target),
        mir::Terminator::Branch {
            then_block,
            else_block,
            ..
        } => {
            out.push(then_block);
            out.push(else_block);
        }
        mir::Terminator::Throw { unwind, .. } | mir::Terminator::Rethrow { unwind } => {
            if let Some(unwind) = unwind {
                out.push(unwind);
            }
        }
        mir::Terminator::Return { .. }
        | mir::Terminator::Resume
        | mir::Terminator::Trap { .. }
        | mir::Terminator::Unreachable => {}
    }
    if let Some(unwind) = block.unwind
        && !out.contains(&unwind)
    {
        out.push(unwind);
    }
    out
}

fn call_uses(call: &mir::Call, uses: &mut HashSet<mir::LocalId>) {
    for arg in &call.args {
        expr_uses(arg, uses);
    }
}

fn expr_uses(expr: &mir::Expr, uses: &mut HashSet<mir::LocalId>) {
    match &expr.kind {
        mir::ExprKind::Local(local) => {
            uses.insert(*local);
        }
        mir::ExprKind::TupleLiteral(elements)
        | mir::ExprKind::ArrayLiteral { elements, .. }
        | mir::ExprKind::StructInit { args: elements, .. }
        | mir::ExprKind::ClassInit { args: elements, .. }
        | mir::ExprKind::ClosureAlloc {
            captures: elements, ..
        }
        | mir::ExprKind::VariantConstruct {
            fields: elements, ..
        } => {
            for element in elements {
                expr_uses(element, uses);
            }
        }
        mir::ExprKind::Retype { operand, .. }
        | mir::ExprKind::ClosureCapture {
            closure: operand, ..
        }
        | mir::ExprKind::ForeignCallbackRegister {
            closure: operand, ..
        }
        | mir::ExprKind::ForeignCallbackOperation {
            callback: operand, ..
        }
        | mir::ExprKind::FieldAccess {
            receiver: operand, ..
        }
        | mir::ExprKind::AtomicFieldLoad {
            object: operand, ..
        }
        | mir::ExprKind::Box(operand)
        | mir::ExprKind::Unbox(operand)
        | mir::ExprKind::IsInstance {
            operand,
            check_ty: _,
        }
        | mir::ExprKind::Cast { operand, .. }
        | mir::ExprKind::ArrayLen { operand, .. }
        | mir::ExprKind::ArrayClone { operand, .. }
        | mir::ExprKind::PtrFromUInt { operand, .. }
        | mir::ExprKind::PtrToUInt(operand)
        | mir::ExprKind::PtrCast { operand, .. }
        | mir::ExprKind::Unary { operand, .. }
        | mir::ExprKind::EnumTag(operand)
        | mir::ExprKind::EnumField { operand, .. } => expr_uses(operand, uses),
        mir::ExprKind::AtomicFieldCompareExchange {
            object,
            expected,
            replacement,
            ..
        } => {
            expr_uses(object, uses);
            expr_uses(expected, uses);
            expr_uses(replacement, uses);
        }
        mir::ExprKind::ArrayGet { array, index, .. }
        | mir::ExprKind::Binary {
            lhs: array,
            rhs: index,
            ..
        } => {
            expr_uses(array, uses);
            expr_uses(index, uses);
        }
        mir::ExprKind::PtrLoad {
            pointer, offset, ..
        } => {
            expr_uses(pointer, uses);
            if let Some(offset) = offset {
                expr_uses(offset, uses);
            }
        }
        mir::ExprKind::PtrStore {
            pointer,
            offset,
            value,
            ..
        } => {
            expr_uses(pointer, uses);
            if let Some(offset) = offset {
                expr_uses(offset, uses);
            }
            expr_uses(value, uses);
        }
        mir::ExprKind::PtrOffset {
            pointer, offset, ..
        } => {
            expr_uses(pointer, uses);
            expr_uses(offset, uses);
        }
        mir::ExprKind::AddressOf { local, .. } => {
            uses.insert(*local);
        }
        mir::ExprKind::StringConst(_)
        | mir::ExprKind::IntLiteral(_)
        | mir::ExprKind::BoolLiteral(_)
        | mir::ExprKind::UnitLiteral
        | mir::ExprKind::GlobalRead(_)
        | mir::ExprKind::GlobalAddress { .. }
        | mir::ExprKind::CaughtException
        | mir::ExprKind::SizeOf(_)
        | mir::ExprKind::AlignOf(_)
        | mir::ExprKind::FunPtrNull(_)
        | mir::ExprKind::FunctionAddress { .. } => {}
    }
}

fn raw<T>(id: la_arena::Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}
