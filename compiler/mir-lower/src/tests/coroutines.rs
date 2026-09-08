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
    assert_eq!(module.interfaces[continuation].name, "Continuation$I32");

    let step = &module.meta.coroutine_steps[coroutine.step];
    assert_eq!(
        step.result(),
        &mir::Type::Integer(mir::IntegerKind::SIGNED_32)
    );
    let step_def = &module.enums[step.enum_id()];
    assert!(step_def.gc_free);
    assert!(step_def.variants.iter().all(|variant| variant.gc_free));
    assert_eq!(
        function.return_ty,
        mir::Type::Enum(step.enum_id(), Vec::new())
    );
    assert_eq!(
        step.completed().definition(&module.enums).unwrap().name,
        "Completed"
    );
    assert_eq!(
        step.completed_payload()
            .definition(&module.enums)
            .unwrap()
            .ty,
        *step.result()
    );
    assert_eq!(
        step.suspended().definition(&module.enums).unwrap().name,
        "Suspended"
    );
    assert!(
        step.suspended()
            .definition(&module.enums)
            .unwrap()
            .fields
            .is_empty()
    );
    let mir::Terminator::Return { value: Some(value) } =
        &function.body.blocks[function.body.entry].terminator
    else {
        panic!("leaf returns a completed step")
    };
    assert!(matches!(
        &value.kind,
        mir::ExprKind::VariantConstruct {
            variant,
            fields,
            ..
        } if *variant == step.completed() && matches!(fields.as_slice(), [field]
            if matches!(field.kind,
                mir::ExprKind::IntegerLiteral(mir::MirIntegerConstant::Signed32(42))))
    ));
}

#[test]
fn generic_unit_return_in_suspend_function_completes_unit() {
    let mut h = Harness::new();
    let unit = h.unit;
    let leaf = identity_fn(&mut h, "genericLeaf");
    h.functions[leaf].is_suspend = true;
    h.instantiate(leaf, vec![unit]);
    let main = empty_main(&mut h);

    let module = lower(&h.finish_coroutines(main));
    assert_eq!(module.validate(), Ok(()));
    let (_, coroutine) = module
        .meta
        .coroutine_functions
        .iter()
        .find(|(_, coroutine)| module.functions[coroutine.function].name == "genericLeaf")
        .expect("genericLeaf<Unit> has coroutine metadata");
    assert_eq!(coroutine.source_return, mir::Type::Unit);
    assert!(matches!(
        &coroutine.lowering,
        mir::CoroutineLowering::Immediate
    ));

    let step = &module.meta.coroutine_steps[coroutine.step];
    assert_eq!(step.result(), &mir::Type::Unit);
    let function = &module.functions[coroutine.function];
    let mir::Terminator::Return { value: Some(value) } =
        &function.body.blocks[function.body.entry].terminator
    else {
        panic!("the immediate Unit coroutine returns Completed(Unit)")
    };
    assert!(matches!(
        &value.kind,
        mir::ExprKind::VariantConstruct {
            variant,
            fields,
            ..
        } if *variant == step.completed()
            && matches!(fields.as_slice(), [field]
                if field.ty == mir::Type::Unit
                    && matches!(field.kind, mir::ExprKind::UnitLiteral))
    ));
}

#[test]
fn suspend_call_generates_a_liveness_based_frame_and_resume_point() {
    let mut h = Harness::new();
    let int = h.int;
    let leaf = h.user_fn_full(
        "leaf",
        Vec::new(),
        Vec::new(),
        int,
        hir::Body {
            locals: Arena::new(),
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(int_lit(&h, 41)),
            })],
        },
    );
    h.functions[leaf].is_suspend = true;
    let mut locals = Arena::new();
    let value = locals.alloc(local("value", int));
    let increment = int_lit(&h, 1);
    let sum = integer_binary(
        &mut h,
        hir::IntegerKind::SIGNED_32,
        hir::NoGcIntegerOperation::Add,
        local_ref(value, int),
        increment,
    );
    let caller = h.user_fn_full(
        "caller",
        Vec::new(),
        Vec::new(),
        int,
        hir::Body {
            locals,
            statements: vec![
                val_decl(value, call_typed(leaf, Vec::new(), int)),
                stmt(hir::StatementKind::Return { value: Some(sum) }),
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
    let fields = module.classes[frame.class()].declared_fields();
    assert_eq!(fields[0].name, "state");
    assert_eq!(
        fields[0].ty,
        mir::Type::MachineScalar(mir::MachineScalarKind::CoroutineFrameState)
    );
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
    let (_, int_slot) = module
        .meta
        .coroutine_slots
        .iter()
        .find(|(_, slot)| slot.value() == &mir::Type::Integer(mir::IntegerKind::SIGNED_32))
        .expect("live Int local uses a concrete coroutine slot");
    let int_slot_def = &module.enums[int_slot.enum_id()];
    assert!(int_slot_def.gc_free);
    assert!(int_slot_def.variants.iter().all(|variant| variant.gc_free));
    assert_eq!(
        int_slot
            .value_variant()
            .definition(&module.enums)
            .unwrap()
            .name,
        "Value"
    );
    assert_eq!(
        int_slot
            .value_payload()
            .definition(&module.enums)
            .unwrap()
            .ty,
        *int_slot.value()
    );
    assert_eq!(
        int_slot.empty().definition(&module.enums).unwrap().name,
        "Empty"
    );
    let throwable = module
        .classes
        .iter()
        .find_map(|(id, definition)| (definition.name == "Throwable").then_some(id))
        .expect("Throwable class");
    let (_, throwable_slot) = module
        .meta
        .coroutine_slots
        .iter()
        .find(|(_, slot)| slot.value() == &mir::Type::Class(throwable))
        .expect("the failure latch uses a concrete Throwable slot");
    let throwable_slot_def = &module.enums[throwable_slot.enum_id()];
    assert!(!throwable_slot_def.gc_free);
    assert!(
        throwable_slot
            .empty()
            .definition(&module.enums)
            .unwrap()
            .gc_free
    );
    assert!(
        !throwable_slot
            .value_variant()
            .definition(&module.enums)
            .unwrap()
            .gc_free
    );
    let point = &module.meta.coroutine_resume_points[resume_points[0]];
    assert_eq!(
        point.result(),
        &mir::Type::Integer(mir::IntegerKind::SIGNED_32)
    );
    assert_eq!(point.site().get(), 1);
    assert_eq!(module.classes[point.adapter()].interfaces.len(), 1);
    assert_eq!(
        module.classes[point.adapter()].declared_fields()[1].ty,
        mir::Type::MachineScalar(mir::MachineScalarKind::CoroutineAdapterState)
    );

    let driver = &module.functions[*driver];
    assert_eq!(
        driver.params[1].ty,
        mir::Type::MachineScalar(mir::MachineScalarKind::CoroutineFrameState)
    );
    for (_, local) in driver.body.locals.iter() {
        let expected = if local.name == "$dispatch_state" || local.name.contains("frame_claim") {
            Some(mir::MachineScalarKind::CoroutineFrameState)
        } else if local.name.contains("adapter_claim") || local.name.contains("registration_claim")
        {
            Some(mir::MachineScalarKind::CoroutineAdapterState)
        } else {
            None
        };
        if let Some(expected) = expected {
            assert_eq!(
                local.ty,
                mir::Type::MachineScalar(expected),
                "compiler-owned state local {} must not be source Int",
                local.name
            );
        }
    }

    let dump = mir::dump(&module);
    assert!(dump.contains("atomic_store_release kind=coroutine-frame-state"));
    assert!(dump.contains("atomic_store_release kind=coroutine-adapter-state"));
    assert!(dump.contains("AtomicCompareExchange kind=coroutine-frame-state"));
    assert!(dump.contains("AtomicCompareExchange kind=coroutine-adapter-state"));
    assert!(dump.contains("MachineScalarLiteral CoroutineFrameState("));
    assert!(dump.contains("MachineScalarLiteral CoroutineAdapterState("));
}

#[test]
fn coroutine_transform_preserves_a_suspending_loop_header_poll_target() {
    let mut h = Harness::new();
    let boolean = h.boolean;
    let unit = h.unit;
    let condition = h.user_fn_full(
        "condition",
        Vec::new(),
        Vec::new(),
        boolean,
        hir::Body {
            locals: Arena::new(),
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(bool_lit(&h, false)),
            })],
        },
    );
    h.functions[condition].is_suspend = true;
    let mut locals = Arena::new();
    let condition_value = locals.alloc(local("condition", boolean));
    let caller = h.user_fn_full(
        "loopingCaller",
        Vec::new(),
        Vec::new(),
        unit,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::While {
                target: hir::LoopId::from_raw(0),
                condition_setup: vec![val_decl(
                    condition_value,
                    call_typed(condition, Vec::new(), boolean),
                )],
                cond: local_ref(condition_value, boolean),
                body: Vec::new(),
            })],
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
    let (_, coroutine) = module
        .meta
        .coroutine_functions
        .iter()
        .find(|(_, coroutine)| module.functions[coroutine.function].name == "loopingCaller")
        .expect("looping caller coroutine metadata");
    let mir::CoroutineLowering::StateMachine { driver, .. } = &coroutine.lowering else {
        panic!("the suspending loop requires a state machine")
    };
    let body = &module.functions[*driver].body;
    let source_headers = body
        .loop_header_polls
        .iter()
        .map(|target| target.header())
        .filter(|target| body.blocks[*target].name.starts_with("while.cond"))
        .collect::<Vec<_>>();
    assert_eq!(source_headers.len(), 1);
    assert!(
        body.blocks
            .iter()
            .any(|(_, block)| block.name.starts_with("coroutine.resume."))
    );
    assert!(
        body.blocks
            .iter()
            .any(|(_, block)| block.name.starts_with("coroutine.post."))
    );
    assert!(body.loop_header_polls.iter().all(|target| {
        let name = &body.blocks[target.header()].name;
        !name.starts_with("coroutine.resume.") && !name.starts_with("coroutine.post.")
    }));
}

#[test]
fn suspend_intrinsic_keeps_machine_kinds_and_generated_loop_header_polls_distinct() {
    let mut h = Harness::new();
    let main = empty_main(&mut h);
    let executable = h.finish_coroutines(main);
    let entry = executable.entry();
    let mut source = executable.into_module();
    let result = module_integer_type(&source, hir::IntegerKind::SIGNED_32);
    let suspend_registration = source.coroutine_core.suspend_registration;
    let registration_ty =
        module_interface_application(&mut source, suspend_registration, vec![result]);
    let Some(suspend_generic) =
        source.functions[source.coroutine_core.suspend_coroutine].generic_definition()
    else {
        panic!("suspendCoroutine is generic")
    };
    let suspend = source.instantiations.alloc(hir::ResolvedGenericFunction {
        generic: suspend_generic,
        type_args: vec![result],
    });
    let mut locals = Arena::new();
    let registration = locals.alloc(local("registration", registration_ty));
    let value = locals.alloc(local("value", result));
    let caller = source.functions.alloc(hir::Function {
        name: "suspendIntrinsicCaller".to_string(),
        access: hir::DeclarationAccess::public(),
        override_access: Vec::new(),
        genericity: hir::FunctionGenericity::Plain,
        is_suspend: true,
        modifiers: hir::CallableModifiers::default(),
        params: vec![param("registration", registration_ty, registration)],
        return_ty: result,
        attributes: hir::FunctionAttributes::default(),
        kind: hir::FunctionKind::User(hir::Body {
            locals,
            statements: vec![
                val_decl(
                    value,
                    generic_call(
                        suspend,
                        vec![local_ref(registration, registration_ty)],
                        result,
                    ),
                ),
                stmt(hir::StatementKind::Return {
                    value: Some(local_ref(value, result)),
                }),
            ],
        }),
        method: None,
        span: SPAN,
    });
    source.top_level.push(caller);

    let source = legacy_executable(source, entry);
    let module = lower(&source);
    let (_, coroutine) = module
        .meta
        .coroutine_functions
        .iter()
        .find(|(_, coroutine)| {
            module.functions[coroutine.function].name == "suspendIntrinsicCaller"
        })
        .expect("intrinsic caller coroutine metadata");
    let mir::CoroutineLowering::StateMachine {
        driver,
        resume_points,
        ..
    } = &coroutine.lowering
    else {
        panic!("suspendCoroutine requires a state machine")
    };
    assert_eq!(resume_points.len(), 1);
    let point = &module.meta.coroutine_resume_points[resume_points[0]];
    assert_eq!(point.site().get(), 1);
    assert_eq!(
        module.classes[point.adapter()].declared_fields()[1].ty,
        mir::Type::MachineScalar(mir::MachineScalarKind::CoroutineAdapterState)
    );

    let driver = &module.functions[*driver];
    for (_, local) in driver.body.locals.iter() {
        if local.name.contains("registration_claim") {
            assert_eq!(
                local.ty,
                mir::Type::MachineScalar(mir::MachineScalarKind::CoroutineAdapterState)
            );
        }
        if local.name.contains("frame_claim") {
            assert_eq!(
                local.ty,
                mir::Type::MachineScalar(mir::MachineScalarKind::CoroutineFrameState)
            );
        }
    }
    assert_eq!(driver.body.loop_header_polls.len(), 2);
    assert!(driver.body.loop_header_polls.iter().any(|target| {
        driver.body.blocks[target.header()]
            .name
            .starts_with("coroutine.completion_wait.")
    }));
    assert!(driver.body.loop_header_polls.iter().any(|target| {
        driver.body.blocks[target.header()]
            .name
            .starts_with("coroutine.registration_wait.")
    }));

    let dump = mir::dump(&module);
    assert!(dump.contains("AtomicLoadAcquire kind=coroutine-adapter-state"));
    assert!(dump.contains("AtomicCompareExchange kind=coroutine-frame-state"));
    assert!(dump.contains("AtomicCompareExchange kind=coroutine-adapter-state"));
    assert!(!dump.contains("$registration_claim.1: Int"));
    assert!(!dump.contains("$registration_frame_claim.1: Int"));
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
    let executable = h.finish_coroutines(main);
    let entry = executable.entry();
    let mut hir_module = executable.into_module();
    let result = module_integer_type(&hir_module, hir::IntegerKind::SIGNED_32);
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

    let hir_module = legacy_executable(hir_module, entry);
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
    assert_eq!(module.interfaces[task_interface].name, "SuspendTask$I32");
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
    let mir::Type::Enum(step_enum, arguments) = &helper.body.locals[step_local].ty else {
        panic!("the helper call result is a CoroutineStep")
    };
    assert!(arguments.is_empty());
    let step_metadata = module
        .meta
        .coroutine_steps
        .iter()
        .find_map(|(_, metadata)| (metadata.enum_id() == *step_enum).then_some(metadata))
        .expect("the helper step has typed metadata");
    let completed_payload = step_metadata.completed_payload();
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
        "Continuation$I32"
    );
    assert!(matches!(resume.args.as_slice(), [completion, field]
            if matches!(completion.kind, mir::ExprKind::Local(_))
                && matches!(&field.kind, mir::ExprKind::EnumField {
                    operand,
                    variant,
                    index
                } if *variant == completed_payload.variant().variant_index()
                    && *index == completed_payload.field_index()
                    && matches!(operand.kind, mir::ExprKind::Local(local) if local == step_local))));

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
