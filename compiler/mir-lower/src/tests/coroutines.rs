//! Coroutine ABI and state-machine lowering.

use super::*;

#[test]
fn suspend_leaf_uses_typed_hidden_abi_and_completed_step() {
    let mut h = Harness::new();
    let leaf = h.user_fn_full(
        "leaf",
        Vec::new(),
        Vec::new(),
        h.int,
        hir::Body {
            locals: Arena::new(),
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(int_lit(&h, 42)),
            })],
        },
    );
    h.functions[leaf].is_suspend = true;
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );

    let module = lower(&h.finish_coroutines(main));
    let (_, coroutine) = module
        .meta
        .coroutine_functions
        .iter()
        .next()
        .expect("one transformed suspend function");
    let function = &module.functions[coroutine.function];
    assert_eq!(function.symbol, "scoop.leaf$suspend");
    assert_eq!(function.params.len(), 1);
    let mir::Type::Interface(continuation) = function.params[0].ty else {
        panic!("hidden completion must be a concrete Continuation<Int>")
    };
    assert_eq!(module.interfaces[continuation].name, "Continuation$I");

    let step = &module.meta.coroutine_steps[coroutine.step];
    assert_eq!(step.result, mir::Type::Int);
    let step_def = &module.enums[step.enum_id];
    assert!(step_def.gc_free);
    assert!(step_def.variants.iter().all(|variant| variant.gc_free));
    assert_eq!(
        function.return_ty,
        mir::Type::Enum(step.enum_id, Vec::new())
    );
    assert_eq!(step_def.variants[0].name, "Completed");
    let mir::Terminator::Return { value: Some(value) } =
        &function.body.blocks[function.body.entry].terminator
    else {
        panic!("leaf returns a completed step")
    };
    assert!(matches!(
        &value.kind,
        mir::ExprKind::VariantConstruct {
            variant: 0,
            fields,
            ..
        } if matches!(fields.as_slice(), [field] if matches!(field.kind, mir::ExprKind::IntLiteral(42)))
    ));
}

#[test]
fn suspend_call_generates_a_liveness_based_frame_and_resume_point() {
    let mut h = Harness::new();
    let leaf = h.user_fn_full(
        "leaf",
        Vec::new(),
        Vec::new(),
        h.int,
        hir::Body {
            locals: Arena::new(),
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(int_lit(&h, 41)),
            })],
        },
    );
    h.functions[leaf].is_suspend = true;
    let mut locals = Arena::new();
    let value = locals.alloc(local("value", h.int));
    let caller = h.user_fn_full(
        "caller",
        Vec::new(),
        Vec::new(),
        h.int,
        hir::Body {
            locals,
            statements: vec![
                val_decl(value, call_typed(leaf, Vec::new(), h.int)),
                stmt(hir::StatementKind::Return {
                    value: Some(primitive_binary(
                        hir::PrimitiveBinaryKind::IntAdd,
                        local_ref(value, h.int),
                        int_lit(&h, 1),
                        h.int,
                    )),
                }),
            ],
        },
    );
    h.functions[caller].is_suspend = true;
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );

    let module = lower(&h.finish_coroutines(main));
    let (_, caller) = module
        .meta
        .coroutine_functions
        .iter()
        .find(|(_, coroutine)| module.functions[coroutine.function].name == "caller")
        .expect("caller coroutine metadata");
    let mir::CoroutineLowering::StateMachine {
        frame,
        driver,
        resume_points,
    } = &caller.lowering
    else {
        panic!("a suspend call requires a state machine")
    };
    assert_eq!(resume_points.len(), 1);
    let frame = &module.meta.coroutine_frames[*frame];
    let fields = module.classes[frame.class].declared_fields();
    assert_eq!(fields[0].name, "state");
    assert_eq!(fields[1].name, "completion");
    assert_eq!(
        fields
            .iter()
            .filter(|field| field.name == "local$value")
            .count(),
        1
    );
    assert!(
        module.functions[*driver]
            .body
            .blocks
            .iter()
            .any(|(_, block)| block.name == "coroutine.resume.1")
    );
    let int_slot = module
        .enums
        .iter()
        .find_map(|(_, definition)| (definition.name == "CoroutineSlot$I").then_some(definition))
        .expect("live Int local uses a concrete coroutine slot");
    assert!(int_slot.gc_free);
    assert!(int_slot.variants.iter().all(|variant| variant.gc_free));
    let throwable = module
        .classes
        .iter()
        .find_map(|(id, definition)| (definition.name == "Throwable").then_some(id))
        .expect("Throwable class");
    let throwable_slot = module
        .enums
        .iter()
        .find_map(|(_, definition)| {
            (definition.name.starts_with("CoroutineSlot$")
                && definition.variants.get(1).is_some_and(|variant| {
                    variant.fields.len() == 1 && variant.fields[0].ty == mir::Type::Class(throwable)
                }))
            .then_some(definition)
        })
        .expect("the failure latch uses a concrete Throwable slot");
    assert!(!throwable_slot.gc_free);
    assert!(throwable_slot.variants[0].gc_free);
    assert!(!throwable_slot.variants[1].gc_free);
    let point = &module.meta.coroutine_resume_points[resume_points[0]];
    assert_eq!(point.result, mir::Type::Int);
    assert_eq!(point.state, 1);
    assert_eq!(module.classes[point.adapter].interfaces.len(), 1);
}

#[test]
fn start_coroutine_resumes_only_an_immediately_completed_task() {
    let mut h = Harness::new();
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let mut hir_module = h.finish_coroutines(main);
    let result = hir_module.int;
    let suspend_task = hir_module.coroutine_core.suspend_task;
    let continuation = hir_module.coroutine_core.continuation;
    let task_ty = module_interface_application(&mut hir_module, suspend_task, vec![result]);
    let completion_ty = module_interface_application(&mut hir_module, continuation, vec![result]);
    let mut locals = Arena::new();
    let task = locals.alloc(local("task", task_ty));
    let completion = locals.alloc(local("completion", completion_ty));
    let Some(start_generic) =
        hir_module.functions[hir_module.coroutine_core.start_coroutine].generic_definition()
    else {
        panic!("startCoroutine is generic")
    };
    let start = hir_module
        .instantiations
        .alloc(hir::ResolvedGenericFunction {
            generic: start_generic,
            type_args: vec![result],
        });
    let launcher = hir_module.functions.alloc(hir::Function {
        name: "launcher".to_string(),
        access: hir::DeclarationAccess::public(),
        override_access: Vec::new(),
        genericity: hir::FunctionGenericity::Plain,
        is_suspend: false,
        modifiers: hir::CallableModifiers::default(),
        params: vec![
            param("task", task_ty, task),
            param("completion", completion_ty, completion),
        ],
        return_ty: hir_module.unit,
        attributes: hir::FunctionAttributes::default(),
        kind: hir::FunctionKind::User(hir::Body {
            locals,
            statements: vec![expr_stmt(expr(
                hir::ExprKind::Call {
                    callee: hir::Callable::Generic(start),
                    args: vec![
                        local_ref(task, task_ty),
                        local_ref(completion, completion_ty),
                    ],
                },
                hir_module.unit,
            ))],
        }),
        method: None,
        span: SPAN,
    });
    hir_module.top_level.push(launcher);

    let module = lower(&hir_module);
    let launcher = module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == "launcher").then_some(function))
        .expect("launcher is lowered");
    let launcher_entry = &launcher.body.blocks[launcher.body.entry];
    let (start, destination) = statement_call(&launcher_entry.statements[0]);
    assert!(destination.is_none(), "startCoroutine returns Unit");
    let mir::Callee::User(helper) = start.target.callee else {
        panic!("startCoroutine lowers to its concrete guarded helper")
    };
    let helper = &module.functions[helper];
    let entry = &helper.body.blocks[helper.body.entry];
    let (run, step_local) = statement_call(&entry.statements[0]);
    let mir::CallKind::Interface {
        interface: task_interface,
        slot: 0,
    } = run.target.kind
    else {
        panic!("startCoroutine must invoke SuspendTask<T>.run through interface dispatch")
    };
    assert_eq!(module.interfaces[task_interface].name, "SuspendTask$I");
    assert_eq!(run.args.len(), 2, "run receives task and hidden completion");
    let step_local = step_local.expect("run returns a CoroutineStep<T>");
    let mir::Terminator::Branch {
        then_block: completed,
        else_block: suspended,
        ..
    } = entry.terminator
    else {
        panic!("startCoroutine must distinguish Completed from Suspended")
    };

    let completed = &helper.body.blocks[completed];
    let (resume, destination) = statement_call(&completed.statements[0]);
    assert!(destination.is_none(), "Continuation.resume returns Unit");
    let mir::CallKind::Interface {
        interface: continuation_interface,
        slot: 0,
    } = resume.target.kind
    else {
        panic!("completed task must resume its outer continuation")
    };
    assert_eq!(
        module.interfaces[continuation_interface].name,
        "Continuation$I"
    );
    assert!(matches!(resume.args.as_slice(), [completion, field]
            if matches!(completion.kind, mir::ExprKind::Local(_))
                && matches!(&field.kind, mir::ExprKind::EnumField {
                    operand,
                    variant: 0,
                    index: 0
                } if matches!(operand.kind, mir::ExprKind::Local(local) if local == step_local))));

    let suspended = &helper.body.blocks[suspended];
    assert!(suspended.statements.is_empty());
    assert!(matches!(
        suspended.terminator,
        mir::Terminator::Return { value: None }
    ));
    let catch_pad = entry
        .unwind
        .expect("task body exceptions enter the guarded helper pad");
    assert!(
        helper.body.blocks[catch_pad]
            .statements
            .iter()
            .any(|statement| {
                matches!(
                    &statement.kind,
                    mir::StatementKind::Call(mir::CallEffect::Value { call, .. })
                        if call.target.callee
                            == mir::Callee::Runtime(mir::RuntimeFn::MaterializeException)
                )
            })
    );
}
