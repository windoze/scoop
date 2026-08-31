//! Typed coroutine state-machine construction over normalized MIR CFG.

use std::collections::{HashMap, HashSet};

use la_arena::Arena;
use scoop_ast::Span;
use scoop_hir as hir;
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
        let throwable = mir::Type::Class(lowerer.class_map[&module.coroutine_core.throwable]);
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
    let throwable_ty = mir::Type::Class(lowerer.class_map[&module.coroutine_core.throwable]);

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
        let (_, slot_ty) =
            lowerer
                .coroutines
                .slot_for(&value_ty, &mut lowerer.enums, &mut lowerer.shell);
        let mir::Type::Enum(slot_enum, _) = slot_ty.clone() else {
            unreachable!("CoroutineSlot is an enum")
        };
        let field = frame_fields.len() as u32;
        frame_fields.push(mir::Field {
            name: format!("local${}", body.locals[*local].name),
            ty: slot_ty,
        });
        frame_slots.insert(
            *local,
            FrameSlot {
                field,
                enum_id: slot_enum,
            },
        );
    }
    let (_, failure_slot_ty) =
        lowerer
            .coroutines
            .slot_for(&throwable_ty, &mut lowerer.enums, &mut lowerer.shell);
    let mir::Type::Enum(failure_slot_enum, _) = failure_slot_ty.clone() else {
        unreachable!("CoroutineSlot is an enum")
    };
    let failure_slot = FrameSlot {
        field: frame_fields.len() as u32,
        enum_id: failure_slot_enum,
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
            failure_slot,
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
            .map(|param| restore_statement(frame_local, param.local, frame_slots[&param.local]))
            .chain(std::iter::once(field_set(
                mir::Expr::Local(frame_local),
                0,
                mir::Expr::IntLiteral(STATE_RUNNING),
            )))
            .collect(),
        terminator: mir::Terminator::Goto(original_entry),
        unwind: None,
    });
    let invalid = protocol_error_block(lowerer, module, &mut body.locals, &mut body.blocks, None);
    let mut next = invalid;
    for (state, target) in resume_targets.iter().rev() {
        next = dispatch_block(&mut body.blocks, frame_local, *state, *target, next);
    }
    let entry = dispatch_block(&mut body.blocks, frame_local, STATE_INITIAL, initial, next);
    body.entry = entry;

    lowerer.functions[driver].params = vec![mir::Param {
        name: "$frame".to_string(),
        ty: mir::Type::Class(frame_class),
        local: frame_local,
    }];
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
const ADAPTER_LATCHED_SUCCESS: i64 = 2;
const ADAPTER_CONSUMED: i64 = 3;
const ADAPTER_LATCHED_FAILURE: i64 = 4;

fn failure_state(state: u32) -> i64 {
    -i64::from(state) - 2
}

#[derive(Clone, Copy)]
struct FrameSlot {
    field: u32,
    enum_id: mir::EnumId,
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
            failure_slot,
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
        destination.map(|local| frame_slots[&local]),
        failure_slot,
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
                frame_local,
                local,
                *slot,
                body.locals[local].ty.clone(),
            ));
        }
    }
    block.statements.push(field_set(
        mir::Expr::Local(frame_local),
        0,
        mir::Expr::IntLiteral(i64::from(site.state)),
    ));
    block
        .statements
        .push(statement(mir::StatementKind::ValDecl {
            local: adapter_local,
            init: mir::Expr::ClassInit {
                class_id: adapter.class,
                args: vec![
                    mir::Expr::Local(frame_local),
                    mir::Expr::IntLiteral(ADAPTER_WAITING),
                ],
            },
        }));
    call.args.push(mir::Expr::Local(adapter_local));
    block.statements.push(statement(mir::StatementKind::Call(
        mir::CallEffect::Value {
            destination: step_local,
            call,
        },
    )));

    let completed = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.completed.{}", site.state),
        statements: {
            let mut statements = vec![field_set(
                mir::Expr::Local(adapter_local),
                1,
                mir::Expr::IntLiteral(ADAPTER_CONSUMED),
            )];
            if let Some(destination) = destination {
                statements.push(statement(mir::StatementKind::ValDecl {
                    local: destination,
                    init: mir::Expr::EnumField {
                        operand: Box::new(mir::Expr::Local(step_local)),
                        variant: 0,
                        index: 0,
                    },
                }));
            }
            statements
        },
        terminator: mir::Terminator::Goto(post),
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
        cond: is_completed(step_local),
        then_block: completed,
        else_block: suspended,
    };

    let mut resume_statements = vec![field_set(
        mir::Expr::Local(frame_local),
        0,
        mir::Expr::IntLiteral(STATE_RUNNING),
    )];
    for local in &site.live_after {
        if Some(*local) == destination {
            continue;
        }
        if let Some(slot) = frame_slots.get(local) {
            resume_statements.push(restore_statement(frame_local, *local, *slot));
        }
    }
    if let Some(destination) = destination {
        resume_statements.push(restore_statement(
            frame_local,
            destination,
            frame_slots[&destination],
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
    let mut statements = vec![field_set(
        mir::Expr::Local(frame),
        0,
        mir::Expr::IntLiteral(STATE_RUNNING),
    )];
    for local in &site.live_after {
        if Some(*local) == site.destination {
            continue;
        }
        if let Some(slot) = frame_slots.get(local) {
            statements.push(restore_statement(frame, *local, *slot));
        }
    }
    body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.failure.{}", site.state),
        statements,
        terminator: mir::Terminator::Throw {
            exception: mir::Expr::EnumField {
                operand: Box::new(frame_field(mir::Expr::Local(frame), failure_slot.field)),
                variant: 1,
                index: 0,
            },
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
    let continuation = lowerer.interfaces.get_or_create(
        module,
        &mut lowerer.shell,
        module.coroutine_core.continuation,
        vec![result.clone()],
    );
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
    if let Some((success, failure)) = safe_latches {
        fields.push(mir::Field {
            name: "result".to_string(),
            ty: mir::Type::Enum(success.enum_id, Vec::new()),
        });
        fields.push(mir::Field {
            name: "failure".to_string(),
            ty: mir::Type::Enum(failure.enum_id, Vec::new()),
        });
    }
    let class = generated_class(lowerer, name, fields, vec![continuation], Vec::new());
    let resume = generate_resume_method(
        lowerer,
        module,
        class,
        destination,
        outer_step,
        outer_continuation,
        outer_resume,
        outer_failure,
        driver,
        source_symbol,
        state,
        result,
        safe_latches.map(|(success, _)| success),
    );
    let failure = generate_failure_method(
        lowerer,
        module,
        class,
        failure_slot,
        outer_step,
        outer_continuation,
        outer_resume,
        outer_failure,
        driver,
        source_symbol,
        state,
        failure_state,
        safe_latches.map(|(_, failure)| failure),
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
    let mut blocks = Arena::new();
    let invalid = protocol_error_block(lowerer, module, &mut locals, &mut blocks, None);
    let exits = drive_exit_blocks(
        lowerer,
        module,
        &mut locals,
        &mut blocks,
        this,
        step,
        outer_continuation,
        outer_resume,
        outer_failure,
    );
    let valid = blocks.alloc(mir::BasicBlock {
        name: "valid".to_string(),
        statements: {
            let mut statements = vec![field_set(
                mir::Expr::Local(this),
                1,
                mir::Expr::IntLiteral(ADAPTER_CONSUMED),
            )];
            if let Some(destination) = destination {
                statements.push(field_set(
                    adapter_frame(this),
                    destination.field,
                    slot_value(destination, mir::Expr::Local(value), result.clone()),
                ));
            }
            statements.push(statement(mir::StatementKind::Call(
                mir::CallEffect::Value {
                    destination: step,
                    call: mir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Direct,
                            callee: mir::Callee::User(driver),
                        },
                        args: vec![adapter_frame(this)],
                    },
                },
            )));
            statements
        },
        terminator: mir::Terminator::Branch {
            cond: is_completed(step),
            then_block: exits.completed,
            else_block: exits.suspended,
        },
        unwind: Some(exits.catch_pad),
    });
    let valid_state = blocks.alloc(mir::BasicBlock {
        name: "valid_state".to_string(),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: int_eq(frame_field(adapter_frame(this), 0), i64::from(state)),
            then_block: valid,
            else_block: invalid,
        },
        unwind: None,
    });
    let waiting = blocks.alloc(mir::BasicBlock {
        name: "waiting".to_string(),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: int_eq(frame_field(mir::Expr::Local(this), 1), ADAPTER_WAITING),
            then_block: valid_state,
            else_block: invalid,
        },
        unwind: None,
    });
    let entry = if let Some(latch) = latch {
        let latched = blocks.alloc(mir::BasicBlock {
            name: "latched".to_string(),
            statements: vec![
                field_set(
                    mir::Expr::Local(this),
                    latch.field,
                    slot_value(latch, mir::Expr::Local(value), result.clone()),
                ),
                field_set(
                    mir::Expr::Local(this),
                    1,
                    mir::Expr::IntLiteral(ADAPTER_LATCHED_SUCCESS),
                ),
            ],
            terminator: mir::Terminator::Return { value: None },
            unwind: None,
        });
        blocks.alloc(mir::BasicBlock {
            name: "entry".to_string(),
            statements: Vec::new(),
            terminator: mir::Terminator::Branch {
                cond: int_eq(frame_field(mir::Expr::Local(this), 1), ADAPTER_REGISTERING),
                then_block: latched,
                else_block: waiting,
            },
            unwind: None,
        })
    } else {
        waiting
    };
    let function = lowerer.functions.alloc(mir::Function {
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
    let throwable = mir::Type::Class(lowerer.class_map[&module.coroutine_core.throwable]);
    let mut locals = Arena::new();
    let this = locals.alloc(local("this", mir::Type::Class(adapter)));
    let exception = locals.alloc(local("exception", throwable.clone()));
    let step = locals.alloc(local("$step", outer_step.clone()));
    let mut blocks = Arena::new();
    let invalid = protocol_error_block(lowerer, module, &mut locals, &mut blocks, None);
    let exits = drive_exit_blocks(
        lowerer,
        module,
        &mut locals,
        &mut blocks,
        this,
        step,
        outer_continuation,
        outer_resume,
        outer_failure,
    );
    let valid = blocks.alloc(mir::BasicBlock {
        name: "valid".to_string(),
        statements: vec![
            field_set(
                mir::Expr::Local(this),
                1,
                mir::Expr::IntLiteral(ADAPTER_CONSUMED),
            ),
            field_set(
                adapter_frame(this),
                failure_slot.field,
                slot_value(failure_slot, mir::Expr::Local(exception), throwable.clone()),
            ),
            field_set(adapter_frame(this), 0, mir::Expr::IntLiteral(failure_state)),
            statement(mir::StatementKind::Call(mir::CallEffect::Value {
                destination: step,
                call: mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(driver),
                    },
                    args: vec![adapter_frame(this)],
                },
            })),
        ],
        terminator: mir::Terminator::Branch {
            cond: is_completed(step),
            then_block: exits.completed,
            else_block: exits.suspended,
        },
        unwind: Some(exits.catch_pad),
    });
    let valid_state = blocks.alloc(mir::BasicBlock {
        name: "valid_state".to_string(),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: int_eq(frame_field(adapter_frame(this), 0), i64::from(state)),
            then_block: valid,
            else_block: invalid,
        },
        unwind: None,
    });
    let waiting = blocks.alloc(mir::BasicBlock {
        name: "waiting".to_string(),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: int_eq(frame_field(mir::Expr::Local(this), 1), ADAPTER_WAITING),
            then_block: valid_state,
            else_block: invalid,
        },
        unwind: None,
    });
    let entry = if let Some(latch) = latch {
        let latched = blocks.alloc(mir::BasicBlock {
            name: "latched".to_string(),
            statements: vec![
                field_set(
                    mir::Expr::Local(this),
                    latch.field,
                    slot_value(latch, mir::Expr::Local(exception), throwable.clone()),
                ),
                field_set(
                    mir::Expr::Local(this),
                    1,
                    mir::Expr::IntLiteral(ADAPTER_LATCHED_FAILURE),
                ),
            ],
            terminator: mir::Terminator::Return { value: None },
            unwind: None,
        });
        blocks.alloc(mir::BasicBlock {
            name: "entry".to_string(),
            statements: Vec::new(),
            terminator: mir::Terminator::Branch {
                cond: int_eq(frame_field(mir::Expr::Local(this), 1), ADAPTER_REGISTERING),
                then_block: latched,
                else_block: waiting,
            },
            unwind: None,
        })
    } else {
        waiting
    };
    let function = lowerer.functions.alloc(mir::Function {
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
    outer_continuation: mir::InterfaceId,
    outer_resume: mir::FunctionId,
    outer_failure: mir::FunctionId,
) -> DriveExitBlocks {
    let throwable = mir::Type::Class(lowerer.class_map[&module.coroutine_core.throwable]);
    let exception = locals.alloc(local("$uncaught", throwable));
    let completed = blocks.alloc(mir::BasicBlock {
        name: "completed".to_string(),
        statements: vec![
            field_set(
                adapter_frame(this),
                0,
                mir::Expr::IntLiteral(STATE_COMPLETED),
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
                    frame_field(adapter_frame(this), 1),
                    mir::Expr::EnumField {
                        operand: Box::new(mir::Expr::Local(step)),
                        variant: 0,
                        index: 0,
                    },
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
            field_set(
                adapter_frame(this),
                0,
                mir::Expr::IntLiteral(STATE_COMPLETED),
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
                    frame_field(adapter_frame(this), 1),
                    mir::Expr::Local(exception),
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
                    args: vec![mir::Expr::CaughtException],
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
        fields,
        base_class: None,
        interfaces,
        vtable: any_vtable(),
        itables,
    });
    let shell = lowerer.shell.classes.alloc(mir::ClassDef {
        modifier: mir::ClassModifier::Final,
        name,
        fields: Vec::new(),
        base_class: None,
        interfaces: Vec::new(),
        vtable: Vec::new(),
        itables: Vec::new(),
    });
    assert_eq!(class, shell, "the mangling shell mirrors class ids");
    class
}

fn any_vtable() -> Vec<mir::TableSlot> {
    vec![
        mir::TableSlot::Runtime(mir::RuntimeFn::AnyEquals),
        mir::TableSlot::Runtime(mir::RuntimeFn::AnyHashCode),
        mir::TableSlot::Runtime(mir::RuntimeFn::AnyToString),
    ]
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
        mir::Expr::IntLiteral(STATE_INITIAL),
        mir::Expr::Local(completion),
    ];
    for old_local in saved {
        let slot = frame_slots[old_local];
        let value_ty = match &locals[params.get(old_local).copied().unwrap_or(completion)].ty {
            ty if params.contains_key(old_local) => ty.clone(),
            _ => mir::Type::Unit,
        };
        args.push(match params.get(old_local) {
            Some(local) => slot_value(slot, mir::Expr::Local(*local), value_ty),
            None => slot_empty(slot),
        });
    }
    args.push(slot_empty(failure_slot));
    let mut blocks = Arena::new();
    let entry = blocks.alloc(mir::BasicBlock {
        name: "entry".to_string(),
        statements: vec![
            statement(mir::StatementKind::ValDecl {
                local: frame,
                init: mir::Expr::ClassInit {
                    class_id: frame_class,
                    args,
                },
            }),
            statement(mir::StatementKind::Call(mir::CallEffect::Value {
                destination: step,
                call: mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(driver),
                    },
                    args: vec![mir::Expr::Local(frame)],
                },
            })),
        ],
        terminator: mir::Terminator::Return {
            value: Some(mir::Expr::Local(step)),
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
    frame: mir::LocalId,
    state: i64,
    target: mir::BlockId,
    otherwise: mir::BlockId,
) -> mir::BlockId {
    blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.dispatch.{state}"),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: int_eq(frame_field(mir::Expr::Local(frame), 0), state),
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
    let class = module.coroutine_core.illegal_state_exception;
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
            exception: mir::Expr::Local(exception),
            unwind,
        },
        unwind,
    })
}

fn save_statement(
    frame: mir::LocalId,
    local: mir::LocalId,
    slot: FrameSlot,
    ty: mir::Type,
) -> mir::Statement {
    field_set(
        mir::Expr::Local(frame),
        slot.field,
        slot_value(slot, mir::Expr::Local(local), ty),
    )
}

fn restore_statement(frame: mir::LocalId, local: mir::LocalId, slot: FrameSlot) -> mir::Statement {
    statement(mir::StatementKind::Assign {
        local,
        value: mir::Expr::EnumField {
            operand: Box::new(frame_field(mir::Expr::Local(frame), slot.field)),
            variant: 1,
            index: 0,
        },
    })
}

fn slot_empty(slot: FrameSlot) -> mir::Expr {
    mir::Expr::VariantConstruct {
        ty: mir::Type::Enum(slot.enum_id, Vec::new()),
        variant: 0,
        fields: Vec::new(),
    }
}

fn slot_value(slot: FrameSlot, value: mir::Expr, _ty: mir::Type) -> mir::Expr {
    mir::Expr::VariantConstruct {
        ty: mir::Type::Enum(slot.enum_id, Vec::new()),
        variant: 1,
        fields: vec![value],
    }
}

fn suspended_value(step: &mir::Type) -> mir::Expr {
    mir::Expr::VariantConstruct {
        ty: step.clone(),
        variant: 1,
        fields: Vec::new(),
    }
}

fn is_completed(step: mir::LocalId) -> mir::Expr {
    int_eq(mir::Expr::EnumTag(Box::new(mir::Expr::Local(step))), 0)
}

fn int_eq(lhs: mir::Expr, rhs: i64) -> mir::Expr {
    mir::Expr::Binary {
        op: mir::BinOp::IntEq,
        lhs: Box::new(lhs),
        rhs: Box::new(mir::Expr::IntLiteral(rhs)),
    }
}

fn adapter_frame(this: mir::LocalId) -> mir::Expr {
    frame_field(mir::Expr::Local(this), 0)
}

fn frame_field(frame: mir::Expr, index: u32) -> mir::Expr {
    mir::Expr::FieldAccess {
        receiver: Box::new(frame),
        index,
    }
}

fn field_set(object: mir::Expr, index: u32, value: mir::Expr) -> mir::Statement {
    statement(mir::StatementKind::FieldSet {
        object,
        index,
        value,
    })
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
        mir::Callee::Closure(function_type) | mir::Callee::FunctionBridge(function_type) => {
            return lowerer.shell.function_types[function_type]
                .return_type
                .clone();
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
    let throwable = mir::Type::Class(lowerer.class_map[&module.coroutine_core.throwable]);
    let (_, result_latch_ty) =
        lowerer
            .coroutines
            .slot_for(&site.result, &mut lowerer.enums, &mut lowerer.shell);
    let (_, failure_latch_ty) =
        lowerer
            .coroutines
            .slot_for(&throwable, &mut lowerer.enums, &mut lowerer.shell);
    let mir::Type::Enum(result_latch_enum, _) = result_latch_ty else {
        unreachable!("CoroutineSlot is an enum")
    };
    let mir::Type::Enum(failure_latch_enum, _) = failure_latch_ty else {
        unreachable!("CoroutineSlot is an enum")
    };
    let result_latch = FrameSlot {
        field: 2,
        enum_id: result_latch_enum,
    };
    let failure_latch = FrameSlot {
        field: 3,
        enum_id: failure_latch_enum,
    };
    let adapter = generate_adapter(
        lowerer,
        module,
        frame_class,
        frame,
        site.destination.map(|local| frame_slots[&local]),
        failure_slot,
        outer_step,
        outer_continuation,
        outer_resume,
        outer_failure,
        source_symbol,
        driver,
        site.state,
        failure_state(site.state),
        &site.result,
        Some((result_latch, failure_latch)),
    );
    let adapter_local = body.locals.alloc(local(
        &format!("$safe_adapter.{}", site.state),
        mir::Type::Class(adapter.class),
    ));
    let exception = body.locals.alloc(local(
        &format!("$resume_exception.{}", site.state),
        throwable,
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
                frame_local,
                local,
                *slot,
                body.locals[local].ty.clone(),
            ));
        }
    }
    current.statements.push(field_set(
        mir::Expr::Local(frame_local),
        0,
        mir::Expr::IntLiteral(i64::from(site.state)),
    ));
    current
        .statements
        .push(statement(mir::StatementKind::ValDecl {
            local: adapter_local,
            init: mir::Expr::ClassInit {
                class_id: adapter.class,
                args: vec![
                    mir::Expr::Local(frame_local),
                    mir::Expr::IntLiteral(ADAPTER_REGISTERING),
                    slot_empty(result_latch),
                    slot_empty(failure_latch),
                ],
            },
        }));
    call.target.callee = mir::Callee::Monomorphized(register);
    call.args.push(mir::Expr::Local(adapter_local));
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
            let mut statements = vec![field_set(
                mir::Expr::Local(adapter_local),
                1,
                mir::Expr::IntLiteral(ADAPTER_CONSUMED),
            )];
            if let Some(destination) = site.destination {
                statements.push(statement(mir::StatementKind::ValDecl {
                    local: destination,
                    init: mir::Expr::EnumField {
                        operand: Box::new(frame_field(
                            mir::Expr::Local(adapter_local),
                            result_latch.field,
                        )),
                        variant: 1,
                        index: 0,
                    },
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
            field_set(
                mir::Expr::Local(adapter_local),
                1,
                mir::Expr::IntLiteral(ADAPTER_CONSUMED),
            ),
            statement(mir::StatementKind::ValDecl {
                local: exception,
                init: mir::Expr::EnumField {
                    operand: Box::new(frame_field(
                        mir::Expr::Local(adapter_local),
                        failure_latch.field,
                    )),
                    variant: 1,
                    index: 0,
                },
            }),
        ],
        terminator: mir::Terminator::Throw {
            exception: mir::Expr::Local(exception),
            unwind,
        },
        unwind,
    });
    let suspended = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registered_suspend.{}", site.state),
        statements: vec![field_set(
            mir::Expr::Local(adapter_local),
            1,
            mir::Expr::IntLiteral(ADAPTER_WAITING),
        )],
        terminator: mir::Terminator::Return {
            value: Some(suspended_value(outer_step)),
        },
        unwind: None,
    });
    let invalid = protocol_error_block(lowerer, module, &mut body.locals, &mut body.blocks, unwind);
    let check_empty = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registered_empty.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                frame_field(mir::Expr::Local(adapter_local), 1),
                ADAPTER_REGISTERING,
            ),
            then_block: suspended,
            else_block: invalid,
        },
        unwind,
    });
    let check_failure = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registered_check_failure.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                frame_field(mir::Expr::Local(adapter_local), 1),
                ADAPTER_LATCHED_FAILURE,
            ),
            then_block: failure,
            else_block: check_empty,
        },
        unwind,
    });
    let registration_propagate = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registration_propagate.{}", site.state),
        statements: vec![field_set(
            mir::Expr::Local(adapter_local),
            1,
            mir::Expr::IntLiteral(ADAPTER_CONSUMED),
        )],
        terminator: mir::Terminator::Throw {
            exception: mir::Expr::Local(exception),
            unwind,
        },
        unwind,
    });
    let registration_protocol = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registration_protocol.{}", site.state),
        statements: vec![field_set(
            mir::Expr::Local(adapter_local),
            1,
            mir::Expr::IntLiteral(ADAPTER_CONSUMED),
        )],
        terminator: mir::Terminator::Goto(invalid),
        unwind,
    });
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
                    args: vec![mir::Expr::CaughtException],
                },
            })),
            statement(mir::StatementKind::Eh(mir::EhStatement::EndCatch)),
        ],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                frame_field(mir::Expr::Local(adapter_local), 1),
                ADAPTER_REGISTERING,
            ),
            then_block: registration_propagate,
            else_block: registration_protocol,
        },
        unwind: None,
    });
    body.blocks[register_call].terminator = mir::Terminator::Branch {
        cond: int_eq(
            frame_field(mir::Expr::Local(adapter_local), 1),
            ADAPTER_LATCHED_SUCCESS,
        ),
        then_block: success,
        else_block: check_failure,
    };
    body.blocks[register_call].unwind = Some(registration_failure);

    let mut resume_statements = vec![field_set(
        mir::Expr::Local(frame_local),
        0,
        mir::Expr::IntLiteral(STATE_RUNNING),
    )];
    for local in &site.live_after {
        if Some(*local) == site.destination {
            continue;
        }
        if let Some(slot) = frame_slots.get(local) {
            resume_statements.push(restore_statement(frame_local, *local, *slot));
        }
    }
    if let Some(destination) = site.destination {
        resume_statements.push(restore_statement(
            frame_local,
            destination,
            frame_slots[&destination],
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
        mir::Callee::Closure(_) | mir::Callee::FunctionBridge(_) => return None,
        mir::Callee::CoroutineSuspend { .. } | mir::Callee::Runtime(_) => return None,
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
        mir::StatementKind::ArraySet {
            array,
            index,
            value,
        } => {
            expr_uses(array, &mut uses);
            expr_uses(index, &mut uses);
            expr_uses(value, &mut uses);
        }
        mir::StatementKind::FieldSet { object, value, .. } => {
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
    match expr {
        mir::Expr::Local(local) => {
            uses.insert(*local);
        }
        mir::Expr::TupleLiteral(elements)
        | mir::Expr::ArrayLiteral(elements)
        | mir::Expr::StructInit { args: elements, .. }
        | mir::Expr::ClassInit { args: elements, .. }
        | mir::Expr::ClosureAlloc {
            captures: elements, ..
        }
        | mir::Expr::VariantConstruct {
            fields: elements, ..
        } => {
            for element in elements {
                expr_uses(element, uses);
            }
        }
        mir::Expr::Retype { operand, .. }
        | mir::Expr::ClosureCapture {
            closure: operand, ..
        }
        | mir::Expr::FieldAccess {
            receiver: operand, ..
        }
        | mir::Expr::Box(operand)
        | mir::Expr::Unbox(operand)
        | mir::Expr::IsInstance {
            operand,
            check_ty: _,
        }
        | mir::Expr::Cast { operand, .. }
        | mir::Expr::ArrayLen(operand)
        | mir::Expr::ArrayClone(operand)
        | mir::Expr::Unary { operand, .. }
        | mir::Expr::EnumTag(operand)
        | mir::Expr::EnumField { operand, .. } => expr_uses(operand, uses),
        mir::Expr::ArrayGet { array, index }
        | mir::Expr::Binary {
            lhs: array,
            rhs: index,
            ..
        } => {
            expr_uses(array, uses);
            expr_uses(index, uses);
        }
        mir::Expr::StringConst(_)
        | mir::Expr::IntLiteral(_)
        | mir::Expr::BoolLiteral(_)
        | mir::Expr::UnitLiteral
        | mir::Expr::CaughtException => {}
    }
}

fn raw<T>(id: la_arena::Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}
