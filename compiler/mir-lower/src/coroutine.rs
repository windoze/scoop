//! Typed coroutine state-machine construction over normalized MIR CFG.

use std::collections::{HashMap, HashSet};

use la_arena::Arena;
use scoop_ast::Span;
use scoop_hir::concrete as hir;
use scoop_mir as mir;

use super::Lowerer;

mod adapters;
mod construction;
mod eh;
mod intrinsics;
mod liveness;
mod sites;

use adapters::*;
use construction::*;
use intrinsics::*;
use liveness::*;
use sites::*;

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
