use super::*;

struct CoroutineFixture {
    module: Module,
    driver: FunctionId,
    success_entry: BlockId,
    failure_entry: BlockId,
}

fn coroutine_fixture(continue_parent: bool) -> CoroutineFixture {
    let (mut module, _) = module_with_variants(Vec::new());
    let result = Type::Integer(IntegerKind::SIGNED_32);
    let throwable = module.classes.alloc(ClassDef {
        modifier: ClassModifier::Open,
        name: "Throwable".to_string(),
        representation: ClassRepresentation::Declared {
            fields: Vec::new(),
            base_class: None,
        },
        interfaces: Vec::new(),
        vtable: Vec::new(),
        itables: Vec::new(),
    });
    let adapter = module.classes.alloc(ClassDef {
        modifier: ClassModifier::Final,
        name: "ContinuationAdapter".to_string(),
        representation: ClassRepresentation::Declared {
            fields: Vec::new(),
            base_class: None,
        },
        interfaces: Vec::new(),
        vtable: Vec::new(),
        itables: Vec::new(),
    });

    let step_enum = module.enums.alloc(EnumDef {
        name: "CoroutineStep<Int>".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: vec![
            variant_def("Completed", vec![result.clone()]),
            variant_def("Suspended", Vec::new()),
        ],
    });
    let completed = MirVariantRef::new(&module.enums, step_enum, 0).unwrap();
    let completed = MirVariantFieldRef::new(&module.enums, completed, 0).unwrap();
    let suspended = MirVariantRef::new(&module.enums, step_enum, 1).unwrap();
    let step = module.meta.coroutine_steps.alloc(
        CoroutineStep::checked(&module.enums, completed, suspended, result.clone()).unwrap(),
    );
    let (saved_slot, saved_slot_ty) = slot(&mut module, "CoroutineSlot<Int>", result.clone());
    let (failure_slot, failure_slot_ty) = slot(
        &mut module,
        "CoroutineSlot<Throwable>",
        Type::Class(throwable),
    );

    let mut wrapper_locals = Arena::new();
    let completion_local = wrapper_locals.alloc(Local {
        name: "$completion".to_string(),
        ty: Type::Boolean,
        mutable: false,
    });
    let wrapper = module.functions.alloc(Function {
        gc_effect: GcEffect::Managed,
        name: "pending".to_string(),
        symbol: "scoop.pending".to_string(),
        params: vec![Param {
            name: "$completion".to_string(),
            ty: Type::Boolean,
            local: completion_local,
        }],
        return_ty: Type::Enum(step_enum, Vec::new()),
        body: Body::unreachable(wrapper_locals),
    });
    let coroutine = module.meta.coroutine_functions.alloc(CoroutineFunction {
        function: wrapper,
        source_return: result.clone(),
        step,
        lowering: CoroutineLowering::Immediate,
    });

    let frame_class = module.classes.alloc(ClassDef {
        modifier: ClassModifier::Final,
        name: "CoroutineFrame$pending".to_string(),
        representation: ClassRepresentation::Declared {
            fields: vec![
                Field {
                    name: "state".to_string(),
                    ty: Type::MachineScalar(MachineScalarKind::CoroutineFrameState),
                },
                Field {
                    name: "completion".to_string(),
                    ty: Type::Boolean,
                },
                Field {
                    name: "return".to_string(),
                    ty: saved_slot_ty,
                },
                Field {
                    name: "failure".to_string(),
                    ty: failure_slot_ty,
                },
            ],
            base_class: None,
        },
        interfaces: Vec::new(),
        vtable: Vec::new(),
        itables: Vec::new(),
    });
    let state_field = CoroutineFrameFieldRef::checked(&module.classes, frame_class, 0).unwrap();
    let completion_field =
        CoroutineFrameFieldRef::checked(&module.classes, frame_class, 1).unwrap();
    let saved_field = CoroutineFrameFieldRef::checked(&module.classes, frame_class, 2).unwrap();
    let failure_field = CoroutineFrameFieldRef::checked(&module.classes, frame_class, 3).unwrap();
    let saved_value = module.meta.coroutine_saved_values.alloc(
        CoroutineSavedValue::checked(
            &module.classes,
            &module.meta.coroutine_slots,
            saved_field,
            saved_slot,
        )
        .unwrap(),
    );
    let failure_value = module.meta.coroutine_failure_values.alloc(
        CoroutineFailureValue::checked(
            &module.classes,
            &module.meta.coroutine_slots,
            failure_field,
            failure_slot,
            throwable,
        )
        .unwrap(),
    );
    let frame = module.meta.coroutine_frames.alloc(
        CoroutineFrame::checked(
            &module.classes,
            &module.meta.coroutine_saved_values,
            &module.meta.coroutine_failure_values,
            frame_class,
            coroutine,
            state_field,
            completion_field,
            vec![saved_value],
            failure_value,
        )
        .unwrap(),
    );

    let resume = callback(&mut module, "resume", adapter, result.clone());
    let resume_failure = callback(
        &mut module,
        "resumeWithException",
        adapter,
        Type::Class(throwable),
    );
    let mut locals = Arena::new();
    let frame_local = locals.alloc(Local {
        name: "$frame".to_string(),
        ty: Type::Class(frame_class),
        mutable: false,
    });
    let state_local = locals.alloc(Local {
        name: "$state".to_string(),
        ty: Type::MachineScalar(MachineScalarKind::CoroutineFrameState),
        mutable: false,
    });
    let mut blocks = Arena::new();
    let post = blocks.alloc(BasicBlock {
        name: "coroutine.post.1".to_string(),
        statements: Vec::new(),
        terminator: Terminator::Unreachable,
        unwind: None,
    });
    let success_entry = blocks.alloc(BasicBlock {
        name: "coroutine.resume.1".to_string(),
        statements: Vec::new(),
        terminator: Terminator::Goto(post),
        unwind: None,
    });
    let slot = &module.meta.coroutine_slots[failure_slot];
    let failure_expr = Expr::new(
        Type::Class(throwable),
        ExprKind::EnumField {
            operand: Box::new(Expr::new(
                Type::Enum(slot.enum_id(), Vec::new()),
                ExprKind::FieldAccess {
                    receiver: Box::new(Expr::local(frame_local, Type::Class(frame_class))),
                    index: failure_field.field_index(),
                },
            )),
            variant: slot.value_payload().variant().variant_index(),
            index: slot.value_payload().field_index(),
        },
    );
    let failure_entry = blocks.alloc(BasicBlock {
        name: "coroutine.failure.1".to_string(),
        statements: Vec::new(),
        terminator: Terminator::Throw {
            exception: failure_expr,
            unwind: None,
        },
        unwind: None,
    });
    let initial = blocks.alloc(BasicBlock {
        name: "coroutine.initial".to_string(),
        statements: Vec::new(),
        terminator: Terminator::Goto(post),
        unwind: None,
    });
    let invalid = blocks.alloc(BasicBlock {
        name: "coroutine.invalid".to_string(),
        statements: Vec::new(),
        terminator: Terminator::Unreachable,
        unwind: None,
    });
    let failure_dispatch = dispatch(
        &mut blocks,
        state_local,
        CoroutineFrameState::ResumeFailure(CoroutineSuspendStateId::new(1).unwrap()),
        failure_entry,
        invalid,
    );
    let success_dispatch = dispatch(
        &mut blocks,
        state_local,
        CoroutineFrameState::Suspended(CoroutineSuspendStateId::new(1).unwrap()),
        success_entry,
        failure_dispatch,
    );
    let entry = dispatch(
        &mut blocks,
        state_local,
        CoroutineFrameState::Initial,
        initial,
        success_dispatch,
    );
    let body = Body {
        locals,
        blocks,
        entry,
        loop_header_polls: Vec::new(),
    };
    let driver = module.functions.alloc(Function {
        gc_effect: GcEffect::Managed,
        name: "pending$drive".to_string(),
        symbol: "scoop.pending$drive".to_string(),
        params: vec![
            Param {
                name: "$frame".to_string(),
                ty: Type::Class(frame_class),
                local: frame_local,
            },
            Param {
                name: "$state".to_string(),
                ty: Type::MachineScalar(MachineScalarKind::CoroutineFrameState),
                local: state_local,
            },
        ],
        return_ty: Type::Enum(step_enum, Vec::new()),
        body,
    });
    let parent = if continue_parent {
        CoroutinePendingTransfer::Continue(CoroutineLoopHeaderTarget::new(post))
    } else {
        CoroutinePendingTransfer::Return(CoroutineReturnTransfer::Saved(saved_value))
    };
    let point = module
        .meta
        .coroutine_resume_points
        .alloc(CoroutineResumePoint::new(
            frame,
            CoroutineSuspendStateId::new(1).unwrap(),
            result,
            adapter,
            resume,
            resume_failure,
            vec![parent],
            CoroutineResumeSuccess::new(
                CoroutineResumeEntryTarget::new(success_entry),
                CoroutineCleanupFallthroughTarget::new(post),
            ),
            CoroutineResumeFailure::new(
                CoroutineResumeEntryTarget::new(failure_entry),
                failure_value,
                None,
            ),
        ));
    module.meta.coroutine_functions[coroutine].lowering = CoroutineLowering::StateMachine {
        frame,
        driver,
        resume_points: vec![point],
    };
    CoroutineFixture {
        module,
        driver,
        success_entry,
        failure_entry,
    }
}

fn slot(module: &mut Module, name: &str, value: Type) -> (CoroutineSlotId, Type) {
    let enumeration = module.enums.alloc(EnumDef {
        name: name.to_string(),
        type_arguments: Vec::new(),
        gc_free: false,
        variants: vec![
            variant_def("Value", vec![value.clone()]),
            variant_def("Empty", Vec::new()),
        ],
    });
    let payload = MirVariantRef::new(&module.enums, enumeration, 0).unwrap();
    let payload = MirVariantFieldRef::new(&module.enums, payload, 0).unwrap();
    let empty = MirVariantRef::new(&module.enums, enumeration, 1).unwrap();
    let slot = module
        .meta
        .coroutine_slots
        .alloc(CoroutineSlot::checked(&module.enums, payload, empty, value).unwrap());
    (slot, Type::Enum(enumeration, Vec::new()))
}

fn callback(module: &mut Module, name: &str, adapter: ClassId, value: Type) -> FunctionId {
    let mut locals = Arena::new();
    let receiver = locals.alloc(Local {
        name: "this".to_string(),
        ty: Type::Class(adapter),
        mutable: false,
    });
    let parameter = locals.alloc(Local {
        name: "value".to_string(),
        ty: value.clone(),
        mutable: false,
    });
    module.functions.alloc(Function {
        gc_effect: GcEffect::Managed,
        name: name.to_string(),
        symbol: format!("scoop.{name}"),
        params: vec![
            Param {
                name: "this".to_string(),
                ty: Type::Class(adapter),
                local: receiver,
            },
            Param {
                name: "value".to_string(),
                ty: value,
                local: parameter,
            },
        ],
        return_ty: Type::Unit,
        body: Body::unreachable(locals),
    })
}

fn dispatch(
    blocks: &mut Arena<BasicBlock>,
    state_local: LocalId,
    state: CoroutineFrameState,
    target: BlockId,
    otherwise: BlockId,
) -> BlockId {
    blocks.alloc(BasicBlock {
        name: format!("coroutine.dispatch.{state}"),
        statements: Vec::new(),
        terminator: Terminator::Branch {
            cond: Expr::machine_eq(
                Expr::local(
                    state_local,
                    Type::MachineScalar(MachineScalarKind::CoroutineFrameState),
                ),
                MachineScalarValue::CoroutineFrameState(state),
            ),
            then_block: target,
            else_block: otherwise,
        },
        unwind: None,
    })
}

#[test]
fn complete_coroutine_metadata_validates_and_dumps_typed_roles() {
    let fixture = coroutine_fixture(false);
    assert_eq!(fixture.module.validate(), Ok(()));

    let dump = dump(&fixture.module);
    assert!(dump.contains("state=field0 completion=field1 saved=[cv0] failure=cx0"));
    assert!(dump.contains("Return(cv0) -> Fallthrough"));
    assert!(dump.contains("ManagedThrow(cx0, unwind=propagate)"));
}

#[test]
fn final_validation_rejects_a_transient_pending_call_context() {
    let mut fixture = coroutine_fixture(false);
    let function = fixture.module.entry;
    let block = fixture.module.functions[function].body.entry;
    fixture.module.functions[function].body.blocks[block]
        .statements
        .push(Statement {
            kind: StatementKind::Call(CallEffect::Unit(Call {
                target: CallTarget {
                    kind: CallKind::Direct,
                    callee: Callee::Runtime(RuntimeFn::GcCollect),
                },
                args: Vec::new(),
                pending: CoroutinePendingContext::Chain(NonEmptyCoroutinePendingChain::new(
                    CoroutinePendingSourceTransfer::Fallthrough(
                        CoroutineCleanupFallthroughTarget::new(block),
                    ),
                    Vec::new(),
                )),
            })),
            span: Span::new(0, 0),
        });
    assert_eq!(
        fixture.module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::FunctionBlock { function, block },
            kind: MirValidationErrorKind::NonRootCoroutinePendingContext,
        })
    );
}

#[test]
fn continue_parent_requires_an_explicit_loop_poll_target() {
    let fixture = coroutine_fixture(true);
    assert!(matches!(
        fixture.module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::InvalidCoroutineMetadata {
                reason: "pending continue target is not an explicit loop-header poll target"
            },
            ..
        })
    ));
}

#[test]
fn driver_dispatch_must_be_derived_exactly_from_resume_points() {
    let mut fixture = coroutine_fixture(false);
    let post = fixture.module.functions[fixture.driver]
        .body
        .blocks
        .iter()
        .find_map(|(id, block)| (block.name == "coroutine.post.1").then_some(id))
        .unwrap();
    let dispatch = fixture.module.functions[fixture.driver]
        .body
        .blocks
        .iter()
        .find_map(|(id, block)| (block.name == "coroutine.dispatch.suspended.1").then_some(id))
        .unwrap();
    let Terminator::Branch { then_block, .. } =
        &mut fixture.module.functions[fixture.driver].body.blocks[dispatch].terminator
    else {
        unreachable!()
    };
    *then_block = post;
    assert!(matches!(
        fixture.module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::InvalidCoroutineMetadata {
                reason: "driver dispatch cases do not exactly match resume-point metadata"
            },
            ..
        })
    ));
}

#[test]
fn failure_entry_must_throw_the_exact_failure_slot() {
    let mut fixture = coroutine_fixture(false);
    let block = &mut fixture.module.functions[fixture.driver].body.blocks[fixture.failure_entry];
    block.terminator = Terminator::Throw {
        exception: Expr::unit(),
        unwind: None,
    };
    assert!(matches!(
        fixture.module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::InvalidCoroutineMetadata {
                reason: "failure resume entry must throw its exact frame failure slot along the typed unwind edge"
            },
            ..
        })
    ));
}

#[test]
fn success_entry_must_target_the_typed_post_block() {
    let mut fixture = coroutine_fixture(false);
    fixture.module.functions[fixture.driver].body.blocks[fixture.success_entry].terminator =
        Terminator::Unreachable;
    assert!(matches!(
        fixture.module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::InvalidCoroutineMetadata {
                reason: "success resume entry must go directly to its typed post target"
            },
            ..
        })
    ));
}
