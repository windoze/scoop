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
    register_test_exact_type(&mut module, &result);
    let throwable = module.classes.alloc(ClassDef {
        type_arguments: Vec::new(),
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
    let continuation = module.interfaces.alloc(InterfaceDef {
        name: "Continuation".to_string(),
        type_arguments: vec![result.clone()],
        methods: Vec::new(),
    });
    register_test_exact_type(&mut module, &Type::Interface(continuation));
    register_test_exact_type(&mut module, &Type::Unit);
    let adapter = module.classes.alloc(ClassDef {
        type_arguments: Vec::new(),
        modifier: ClassModifier::Final,
        name: "ContinuationAdapter".to_string(),
        representation: ClassRepresentation::Declared {
            fields: Vec::new(),
            base_class: None,
        },
        interfaces: vec![continuation],
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
        CoroutineStep::checked(
            &module.enums,
            completed,
            suspended,
            result.clone(),
            test_step_identity(&result),
        )
        .unwrap(),
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
        params: vec![Param {
            name: "$completion".to_string(),
            ty: Type::Boolean,
            local: completion_local,
        }],
        return_ty: Type::Enum(step_enum, Vec::new()),
        body: Body::unreachable(wrapper_locals),
    });
    module.entry = wrapper;
    let source = test_source_materialization();
    let source_signature = scoop_identity::ExactCallableSignature::new(
        scoop_identity::Effect::Suspend,
        None,
        Vec::new(),
        test_exact_type(&result).id(),
    );
    module.meta.source_callable_materializations = SourceCallableMaterializations::checked(vec![
        SourceCallableMaterialization::new(wrapper, source, source_signature.clone(), None)
            .unwrap(),
    ])
    .unwrap();
    let coroutine = module.meta.coroutine_functions.alloc(CoroutineFunction {
        function: wrapper,
        source,
        source_odr_group: None,
        logical_signature: source_signature.clone(),
        source_return: result.clone(),
        step,
        lowering: CoroutineLowering::Immediate,
    });

    let frame_class = module.classes.alloc(ClassDef {
        type_arguments: Vec::new(),
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
    let saved_identity = test_local_value(source, 0);
    let frame_identity =
        CoroutineFrameIdentity::new(source, vec![saved_identity.clone()], None).unwrap();
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
            frame_identity,
        )
        .unwrap(),
    );
    module.classes[adapter].representation = ClassRepresentation::Declared {
        fields: vec![
            Field {
                name: "frame".to_string(),
                ty: Type::Class(frame_class),
            },
            Field {
                name: "status".to_string(),
                ty: Type::MachineScalar(MachineScalarKind::CoroutineAdapterState),
            },
        ],
        base_class: None,
    };

    let resume = callback(&mut module, "resume", adapter, result.clone());
    let resume_failure = callback(
        &mut module,
        "resumeWithException",
        adapter,
        Type::Class(throwable),
    );
    module.interfaces[continuation].methods = vec![resume, resume_failure];
    module.classes[adapter].itables = vec![ItableRecord {
        interface: continuation,
        slots: vec![
            TableSlot::Function(resume),
            TableSlot::Function(resume_failure),
        ],
    }];
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
    let saved_local = locals.alloc(Local {
        name: "$saved".to_string(),
        ty: result.clone(),
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
    module.meta.local_values = LocalValueIdentities::checked(
        module
            .meta
            .local_values
            .iter()
            .cloned()
            .chain(std::iter::once(LocalValueIdentity::from_hir(
                driver,
                saved_local,
                saved_identity,
            )))
            .collect(),
    )
    .unwrap();
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
            result.clone(),
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
            {
                let (success_signature, failure_signature) = test_continuation_signatures(
                    &result,
                    &Type::Interface(continuation),
                    &Type::Class(throwable),
                );
                ContinuationAdapterIdentity::direct(
                    source,
                    scoop_identity::StructuralDefinitionPath::from_first(
                        scoop_identity::StructuralPathSegment::new(
                            scoop_identity::StructuralDefinitionSiteRole::CoroutineTransform,
                            0,
                        ),
                        [],
                    ),
                    success_signature,
                    failure_signature,
                    None,
                )
                .unwrap()
            },
        ));
    module.meta.coroutine_functions[coroutine].lowering = CoroutineLowering::StateMachine {
        frame,
        driver,
        driver_identity: Box::new(
            CoroutineDriverIdentity::new(source, None, source_signature).unwrap(),
        ),
        resume_points: vec![point],
    };
    install_generated_exact_types(&mut module);
    install_generated_callables(&mut module);
    CoroutineFixture {
        module,
        driver,
        success_entry,
        failure_entry,
    }
}

fn slot(module: &mut Module, name: &str, value: Type) -> (CoroutineSlotId, Type) {
    register_test_exact_type(module, &value);
    let enumeration = module.enums.alloc(EnumDef {
        name: name.to_string(),
        type_arguments: Vec::new(),
        gc_free: false,
        variants: vec![
            variant_def("Empty", Vec::new()),
            variant_def("Value", vec![value.clone()]),
        ],
    });
    let empty = MirVariantRef::new(&module.enums, enumeration, 0).unwrap();
    let payload = MirVariantRef::new(&module.enums, enumeration, 1).unwrap();
    let payload = MirVariantFieldRef::new(&module.enums, payload, 0).unwrap();
    let slot = module.meta.coroutine_slots.alloc(
        CoroutineSlot::checked(
            &module.enums,
            payload,
            empty,
            value.clone(),
            test_slot_identity(&value),
        )
        .unwrap(),
    );
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
    assert!(dump.contains("environment_id="));
    assert!(dump.contains("success_id="));
    assert!(dump.contains("failure_id="));
    assert!(dump.contains("Return(cv0) -> Fallthrough"));
    assert!(dump.contains("ManagedThrow(cx0, unwind=propagate)"));
}

#[test]
fn continuation_adapter_fields_must_match_its_identity_storage() {
    let mut fixture = coroutine_fixture(false);
    let (_, point) = fixture
        .module
        .meta
        .coroutine_resume_points
        .iter()
        .next()
        .unwrap();
    let adapter = point.adapter();
    fixture.module.classes[adapter].declared_fields_mut()[0].ty = Type::Unit;

    assert!(matches!(
        fixture.module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::InvalidCoroutineMetadata {
                reason: "continuation adapter does not have its exact frame, state, and latch field layout"
            },
            ..
        })
    ));
}

#[test]
fn continuation_adapter_identity_must_name_its_coroutine_source() {
    let mut fixture = coroutine_fixture(false);
    let (point_id, point) = fixture
        .module
        .meta
        .coroutine_resume_points
        .iter()
        .next()
        .unwrap();
    let replacement = CoroutineResumePoint::new(
        point.frame(),
        point.site(),
        point.result().clone(),
        point.adapter(),
        point.resume(),
        point.resume_with_exception(),
        point.parents().to_vec(),
        point.success(),
        point.failure(),
        ContinuationAdapterIdentity::direct(
            test_source_materialization_named("otherSource"),
            point.identity().suspension_site().clone(),
            point
                .identity()
                .success()
                .signature_record()
                .signature()
                .clone(),
            point
                .identity()
                .failure()
                .signature_record()
                .signature()
                .clone(),
            None,
        )
        .unwrap(),
    );
    fixture.module.meta.coroutine_resume_points[point_id] = replacement;

    assert!(matches!(
        fixture.module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::InvalidCoroutineMetadata {
                reason: "continuation-adapter identity does not match its exact source, suspension site, and logical signatures"
            },
            ..
        })
    ));
}

#[test]
fn continuation_adapter_identity_must_retain_its_protocol_signatures() {
    let mut fixture = coroutine_fixture(false);
    let (point_id, point) = fixture
        .module
        .meta
        .coroutine_resume_points
        .iter()
        .next()
        .unwrap();
    let success = point.identity().success().signature_record().signature();
    let failure = point.identity().failure().signature_record().signature();
    let replacement = CoroutineResumePoint::new(
        point.frame(),
        point.site(),
        point.result().clone(),
        point.adapter(),
        point.resume(),
        point.resume_with_exception(),
        point.parents().to_vec(),
        point.success(),
        point.failure(),
        ContinuationAdapterIdentity::direct(
            point.identity().source(),
            point.identity().suspension_site().clone(),
            scoop_identity::ExactCallableSignature::new(
                scoop_identity::Effect::Ordinary,
                success.receiver().into_option(),
                failure.parameters().to_vec(),
                success.result(),
            ),
            failure.clone(),
            None,
        )
        .unwrap(),
    );
    fixture.module.meta.coroutine_resume_points[point_id] = replacement;

    assert!(matches!(
        fixture.module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::InvalidCoroutineMetadata {
                reason: "continuation-adapter identity does not match its exact source, suspension site, and logical signatures"
            },
            ..
        })
    ));
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
fn driver_identity_must_name_the_exact_coroutine_source() {
    let mut fixture = coroutine_fixture(false);
    let (_, coroutine) = fixture
        .module
        .meta
        .coroutine_functions
        .iter_mut()
        .next()
        .unwrap();
    let CoroutineLowering::StateMachine {
        driver_identity, ..
    } = &mut coroutine.lowering
    else {
        unreachable!()
    };
    let signature = driver_identity.signature_record().signature().clone();
    **driver_identity = CoroutineDriverIdentity::new(
        test_source_materialization_named("otherSource"),
        None,
        signature,
    )
    .unwrap();

    assert!(matches!(
        fixture.module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::InvalidCoroutineMetadata {
                reason: "coroutine driver identity does not match its exact source materialization"
            },
            ..
        })
    ));
}

#[test]
fn driver_identity_must_retain_the_source_logical_signature() {
    let mut fixture = coroutine_fixture(false);
    let (_, coroutine) = fixture
        .module
        .meta
        .coroutine_functions
        .iter_mut()
        .next()
        .unwrap();
    let CoroutineLowering::StateMachine {
        driver_identity, ..
    } = &mut coroutine.lowering
    else {
        unreachable!()
    };
    let source_signature = driver_identity.signature_record().signature();
    **driver_identity = CoroutineDriverIdentity::new(
        coroutine.source,
        coroutine.source_odr_group,
        scoop_identity::ExactCallableSignature::new(
            scoop_identity::Effect::Suspend,
            source_signature.receiver().into_option(),
            vec![source_signature.result()],
            source_signature.result(),
        ),
    )
    .unwrap();

    assert!(matches!(
        fixture.module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::InvalidCoroutineMetadata {
                reason: "coroutine driver identity does not match its exact source materialization"
            },
            ..
        })
    ));
}

#[test]
fn callable_signature_relation_must_be_complete() {
    let mut fixture = coroutine_fixture(false);
    fixture.module.meta.callable_signatures = MirCallableSignatures::default();

    assert_eq!(
        fixture.module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::CallableSignature { entry: 0 },
            kind: MirValidationErrorKind::InvalidCallableSignature {
                reason: "the relation is missing a callable signature",
            },
        })
    );
}

#[test]
fn callable_signature_relation_must_match_transform_metadata() {
    let mut fixture = coroutine_fixture(false);
    let mut records = fixture
        .module
        .meta
        .callable_signatures
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let first = &records[0];
    let signature = first.signature();
    records[0] = CallableSignatureRecord::new(
        first.subject(),
        scoop_identity::ExactCallableSignature::new(
            match signature.effect() {
                scoop_identity::Effect::Ordinary => scoop_identity::Effect::Suspend,
                scoop_identity::Effect::Suspend => scoop_identity::Effect::Ordinary,
            },
            signature.receiver().into_option(),
            signature.parameters().to_vec(),
            signature.result(),
        ),
    );
    fixture.module.meta.callable_signatures = MirCallableSignatures::checked(records).unwrap();

    assert_eq!(
        fixture.module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::CallableSignature { entry: 0 },
            kind: MirValidationErrorKind::InvalidCallableSignature {
                reason: "the relation records a different signature for the callable subject",
            },
        })
    );
}

#[test]
fn coroutine_source_must_retain_a_complete_suspend_signature() {
    let mut fixture = coroutine_fixture(false);
    let (_, coroutine) = fixture
        .module
        .meta
        .coroutine_functions
        .iter_mut()
        .next()
        .unwrap();
    coroutine.logical_signature = scoop_identity::ExactCallableSignature::new(
        scoop_identity::Effect::Ordinary,
        None,
        Vec::new(),
        test_exact_type(&coroutine.source_return).id(),
    );

    assert!(matches!(
        fixture.module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::InvalidCoroutineMetadata {
                reason: "coroutine source has no exact suspend logical signature"
            },
            ..
        })
    ));
}

#[test]
fn frame_saved_identity_must_name_one_driver_local() {
    let mut fixture = coroutine_fixture(false);
    fixture.module.meta.local_values = LocalValueIdentities::default();

    assert!(matches!(
        fixture.module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::InvalidCoroutineMetadata {
                reason: "each saved field identity must name exactly one local in its coroutine driver"
            },
            ..
        })
    ));
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
