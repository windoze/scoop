use super::*;

pub(super) fn generated_class(
    lowerer: &mut Lowerer,
    name: String,
    fields: Vec<mir::Field>,
    interfaces: Vec<mir::InterfaceId>,
    itables: Vec<mir::ItableRecord>,
) -> mir::ClassId {
    let class = lowerer.classes.alloc(mir::ClassDef {
        modifier: mir::ClassModifier::Final,
        name: name.clone(),
        type_arguments: Vec::new(),
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
        type_arguments: Vec::new(),
        representation: mir::ClassRepresentation::Declared {
            fields: Vec::new(),
            base_class: None,
        },
        interfaces: Vec::new(),
        vtable: Vec::new(),
        itables: Vec::new(),
    });
    assert_eq!(class, shell, "the type context mirrors class ids");
    class
}

pub(super) fn wrapper_params(
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
pub(super) fn wrapper_body(
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
        frame_state(STATE_INITIAL),
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
    let mut statements = initialized_generated_class(frame, frame_class, args);
    statements.push(statement(mir::StatementKind::Call(
        mir::CallEffect::Value {
            destination: step,
            call: mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: mir::Callee::User(driver),
                },
                args: vec![
                    mir::Expr::local(frame, mir::Type::Class(frame_class)),
                    frame_state(STATE_INITIAL),
                ],
                pending: mir::CoroutinePendingContext::Root,
            },
        },
    )));
    let entry = blocks.alloc(mir::BasicBlock {
        name: "entry".to_string(),
        statements,
        terminator: mir::Terminator::Return {
            value: Some(mir::Expr::local(step, step_ty.clone())),
        },
        unwind: None,
    });
    mir::Body {
        locals,
        blocks,
        entry,
        loop_header_polls: Vec::new(),
    }
}

pub(super) fn dispatch_block(
    blocks: &mut Arena<mir::BasicBlock>,
    dispatch_state: mir::LocalId,
    state: mir::CoroutineFrameState,
    target: mir::BlockId,
    otherwise: mir::BlockId,
) -> mir::BlockId {
    blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.dispatch.{state}"),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: machine_eq(
                mir::Expr::local(
                    dispatch_state,
                    mir::Type::MachineScalar(mir::MachineScalarKind::CoroutineFrameState),
                ),
                mir::MachineScalarValue::CoroutineFrameState(state),
            ),
            then_block: target,
            else_block: otherwise,
        },
        unwind: None,
    })
}

pub(super) fn protocol_error_block(
    lowerer: &Lowerer,
    module: &hir::Module,
    locals: &mut Arena<mir::Local>,
    blocks: &mut Arena<mir::BasicBlock>,
    unwind: Option<mir::BlockId>,
) -> mir::BlockId {
    let class = module.exception_core.illegal_state_exception.class();
    let constructor = module.exception_core.illegal_state_exception.callable();
    let mir_class = lowerer.class_map[&class];
    let exception = locals.alloc(local("$protocol_error", mir::Type::Class(mir_class)));
    blocks.alloc(mir::BasicBlock {
        name: "coroutine.protocol_error".to_string(),
        statements: vec![
            statement(mir::StatementKind::ValDecl {
                local: exception,
                init: mir::Expr::new(
                    mir::Type::Class(mir_class),
                    mir::ExprKind::ClassAlloc {
                        class_id: mir_class,
                    },
                ),
            }),
            statement(mir::StatementKind::Call(mir::CallEffect::Unit(mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: mir::Callee::User(lowerer.ctors[&constructor]),
                },
                args: vec![mir::Expr::local(exception, mir::Type::Class(mir_class))],
                pending: mir::CoroutinePendingContext::Root,
            }))),
        ],
        terminator: mir::Terminator::Throw {
            exception: mir::Expr::local(exception, mir::Type::Class(mir_class)),
            unwind,
        },
        unwind,
    })
}

pub(super) fn save_statement(
    frame: mir::Expr,
    value: mir::Expr,
    slot: &FrameSlot,
) -> mir::Statement {
    field_set(frame, slot.field, slot_value(slot, value))
}

pub(super) fn restore_statement(
    frame: mir::Expr,
    local: mir::LocalId,
    slot: &FrameSlot,
) -> mir::Statement {
    statement(mir::StatementKind::Assign {
        local,
        value: mir::Expr::new(
            slot.value_ty.clone(),
            mir::ExprKind::EnumField {
                operand: Box::new(frame_field(frame, slot.field, slot.slot_ty.clone())),
                variant: slot.value_payload.variant().variant_index(),
                index: slot.value_payload.field_index(),
            },
        ),
    })
}

pub(super) fn slot_empty(slot: &FrameSlot) -> mir::Expr {
    mir::Expr::new(
        slot.slot_ty.clone(),
        mir::ExprKind::VariantConstruct {
            variant: slot.empty,
            fields: Vec::new(),
        },
    )
}

pub(super) fn slot_value(slot: &FrameSlot, value: mir::Expr) -> mir::Expr {
    debug_assert_eq!(value.ty, slot.value_ty);
    mir::Expr::new(
        slot.slot_ty.clone(),
        mir::ExprKind::VariantConstruct {
            variant: slot.value_payload.variant(),
            fields: vec![value],
        },
    )
}

pub(super) fn suspended_value(step: &mir::Type, suspended: mir::MirVariantRef) -> mir::Expr {
    debug_assert_eq!(step, &mir::Type::Enum(suspended.enum_id(), Vec::new()));
    mir::Expr::new(
        step.clone(),
        mir::ExprKind::VariantConstruct {
            variant: suspended,
            fields: Vec::new(),
        },
    )
}

pub(super) fn is_completed(
    step: mir::LocalId,
    step_ty: mir::Type,
    completed: mir::MirVariantRef,
) -> mir::Expr {
    debug_assert_eq!(step_ty, mir::Type::Enum(completed.enum_id(), Vec::new()));
    machine_eq(
        mir::Expr::enum_tag(mir::Expr::local(step, step_ty)),
        mir::MachineScalarValue::EnumTag(completed.variant_index()),
    )
}

pub(super) fn machine_eq(lhs: mir::Expr, rhs: mir::MachineScalarValue) -> mir::Expr {
    mir::Expr::machine_eq(lhs, rhs)
}

pub(super) fn frame_state(state: mir::CoroutineFrameState) -> mir::Expr {
    mir::Expr::machine_scalar(frame_state_value(state))
}

pub(super) fn adapter_state(state: mir::CoroutineAdapterState) -> mir::Expr {
    mir::Expr::machine_scalar(adapter_state_value(state))
}

pub(super) const fn frame_state_value(state: mir::CoroutineFrameState) -> mir::MachineScalarValue {
    mir::MachineScalarValue::CoroutineFrameState(state)
}

pub(super) const fn adapter_state_value(
    state: mir::CoroutineAdapterState,
) -> mir::MachineScalarValue {
    mir::MachineScalarValue::CoroutineAdapterState(state)
}

pub(super) fn adapter_frame(
    this: mir::LocalId,
    adapter: mir::ClassId,
    frame: mir::ClassId,
) -> mir::Expr {
    frame_field(
        mir::Expr::local(this, mir::Type::Class(adapter)),
        0,
        mir::Type::Class(frame),
    )
}

pub(super) fn frame_field(frame: mir::Expr, index: u32, ty: mir::Type) -> mir::Expr {
    mir::Expr::new(
        ty,
        mir::ExprKind::FieldAccess {
            receiver: Box::new(frame),
            index,
        },
    )
}

pub(super) fn field_set(object: mir::Expr, index: u32, value: mir::Expr) -> mir::Statement {
    statement(mir::StatementKind::FieldSet {
        object,
        index,
        value,
    })
}

/// Allocate and initialize a compiler-generated class whose fields are already
/// represented as an ordered concrete payload. Source classes always run their
/// typed initializer functions instead.
pub(super) fn initialized_generated_class(
    local: mir::LocalId,
    class: mir::ClassId,
    fields: Vec<mir::Expr>,
) -> Vec<mir::Statement> {
    let ty = mir::Type::Class(class);
    let mut statements = vec![statement(mir::StatementKind::ValDecl {
        local,
        init: mir::Expr::new(ty.clone(), mir::ExprKind::ClassAlloc { class_id: class }),
    })];
    statements.extend(fields.into_iter().enumerate().map(|(index, value)| {
        field_set(
            mir::Expr::local(local, ty.clone()),
            u32::try_from(index).expect("generated class field index fits u32"),
            value,
        )
    }));
    statements
}

pub(super) fn atomic_field_load(
    object: mir::Expr,
    index: u32,
    kind: mir::MachineScalarKind,
) -> mir::Expr {
    assert!(
        kind.is_atomic_state(),
        "only coroutine state fields are atomic"
    );
    mir::Expr::new(
        mir::Type::MachineScalar(kind),
        mir::ExprKind::AtomicFieldLoad {
            kind,
            object: Box::new(object),
            index,
        },
    )
}

pub(super) fn atomic_field_store(
    object: mir::Expr,
    index: u32,
    value: mir::Expr,
) -> mir::Statement {
    let mir::Type::MachineScalar(kind) = &value.ty else {
        panic!("an atomic state store requires a machine scalar value")
    };
    let kind = *kind;
    assert!(
        kind.is_atomic_state(),
        "only coroutine state fields are atomic"
    );
    statement(mir::StatementKind::AtomicFieldStore {
        kind,
        object,
        index,
        value,
    })
}

pub(super) fn atomic_field_compare_exchange(
    object: mir::Expr,
    index: u32,
    expected: mir::MachineScalarValue,
    replacement: mir::MachineScalarValue,
) -> mir::Expr {
    let kind = expected.kind();
    assert_eq!(
        replacement.kind(),
        kind,
        "compare-exchange states have the same semantic kind"
    );
    assert!(
        kind.is_atomic_state(),
        "only coroutine state fields are atomic"
    );
    mir::Expr::new(
        mir::Type::MachineScalar(kind),
        mir::ExprKind::AtomicFieldCompareExchange {
            kind,
            object: Box::new(object),
            index,
            expected: Box::new(mir::Expr::machine_scalar(expected)),
            replacement: Box::new(mir::Expr::machine_scalar(replacement)),
        },
    )
}

pub(super) fn statement(kind: mir::StatementKind) -> mir::Statement {
    mir::Statement {
        kind,
        span: Span { start: 0, end: 0 },
    }
}

pub(super) fn local(name: &str, ty: mir::Type) -> mir::Local {
    mir::Local {
        name: name.to_string(),
        ty,
        mutable: false,
    }
}

pub(super) fn callee_return_type(lowerer: &Lowerer, callee: mir::Callee) -> mir::Type {
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
                    .step_type_for(&lowerer.source_exact_types, &signature.return_type)
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
