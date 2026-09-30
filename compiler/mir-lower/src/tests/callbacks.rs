use super::*;

#[test]
fn static_no_gc_callback_bridge_keeps_its_source_and_generated_identity() {
    let mut h = Harness::new();
    let target = h.user_fn_full(
        "staticCallbackTarget",
        Vec::new(),
        Vec::new(),
        h.unit,
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    h.functions[target].attributes.gc_effect = hir::GcEffect::NoGc;
    let main = empty_main(&mut h);
    let executable = h.finish(main);
    let entry = executable.entry();
    let mut source = executable.into_module();
    let function_type = hir::FunctionTypeId::from_raw(
        u32::try_from(source.function_types.len())
            .expect("function type id fits u32")
            .into(),
    );
    let managed_function = source.types.alloc(hir::Type::Function(function_type));
    assert_eq!(
        source.function_types.alloc(hir::FunctionType {
            canonical_type: managed_function,
            is_suspend: false,
            parameter_types: Vec::new(),
            return_type: source.unit,
        }),
        function_type
    );
    let function_pointer = source.types.alloc(hir::Type::FunPtr(function_type));
    source.type_identities = rebuild_type_identities(&source);
    let hir::FunctionKind::User(main_body) = &mut source.functions[main].kind else {
        panic!("main is a user function")
    };
    main_body.statements.push(expr_stmt(expr(
        hir::ExprKind::FunctionAddress(target),
        function_pointer,
    )));

    let export = executable_output(source, entry);
    let concrete =
        scoop_hir_lower::concretize_output(&export).expect("concrete type applications are valid");
    let module = crate::lower(&concrete)
        .expect("test LocalConcrete HIR carries locally defined core protocols");
    let (_, bridge) = module
        .callback_bridges
        .iter()
        .next()
        .expect("one function address creates one static callback bridge");
    let source_materialization = module
        .meta
        .source_callable_materializations
        .get(bridge.source)
        .expect("the callback source retains its local-concrete materialization")
        .materialization();

    assert_eq!(bridge.identity().source(), source_materialization);
    assert!(matches!(
        bridge.identity().callable_record().key(),
        scoop_identity::GeneratedCallableKey::StaticNoGcCallbackStorageBridge {
            source,
            signature,
        } if *source == source_materialization
            && signature == bridge.identity().signature_record().signature()
    ));
    let generated = module
        .meta
        .generated_callables
        .get(bridge.bridge_function)
        .expect("the static callback bridge has one generated callable location");
    assert_eq!(
        generated.identity_record(),
        bridge.identity().callable_record()
    );
    assert!(matches!(
        bridge.identity().signature_record().subject(),
        mir::CallableSignatureSubject::Strong(scoop_identity::CallableOwner::Generated(found))
            if found == bridge.identity().callable_record().id()
    ));
    assert_eq!(module.validate(), Ok(()));
}

#[test]
fn nonzero_ulong_pointer_conversion_keeps_its_proof_in_mir() {
    let mut h = Harness::new();
    let pointee = h.int;
    let pointer = h.types.alloc(hir::Type::Ptr(pointee));
    let ulong = h.ulong();
    let mut locals = Arena::new();
    let pointer_local = locals.alloc(local("pointer", pointer));
    let word_local = locals.alloc(local("word", ulong));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    pointer_local,
                    expr(
                        hir::ExprKind::PtrFromNonZeroULong(Box::new(integer_lit(
                            &h,
                            hir::IntegerKind::UNSIGNED_64,
                            1,
                        ))),
                        pointer,
                    ),
                ),
                val_decl(
                    word_local,
                    expr(
                        hir::ExprKind::PtrToULong(Box::new(local_ref(pointer_local, pointer))),
                        ulong,
                    ),
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));
    let statements = entry_statements(
        &module.functions[module
            .output
            .executable_entry()
            .expect("test module is executable")]
        .body,
    );
    let mir::StatementKind::ValDecl {
        init:
            mir::Expr {
                kind:
                    mir::ExprKind::PtrFromNonZeroULong {
                        operand,
                        pointee: mir_pointee,
                    },
                ..
            },
        ..
    } = &statements[0].kind
    else {
        panic!("nonzero ULong conversion must retain its dedicated MIR variant")
    };
    assert_eq!(
        **mir_pointee,
        mir::Type::Integer(mir::IntegerKind::SIGNED_32)
    );
    assert_eq!(
        operand.ty,
        mir::Type::Integer(mir::IntegerKind::UNSIGNED_64)
    );
    assert!(matches!(
        operand.kind,
        mir::ExprKind::IntegerLiteral(value) if value.raw_bits() == 1
    ));
    assert!(matches!(
        statements[1].kind,
        mir::StatementKind::ValDecl {
            init: mir::Expr {
                kind: mir::ExprKind::PtrToULong(_),
                ..
            },
            ..
        }
    ));
}

#[test]
fn foreign_callback_adapter_uses_typed_status_and_argument_offsets() {
    let mut h = Harness::new();
    let int = h.int;
    let unit = h.unit;
    let managed_canonical_type = hir::TypeId::from_raw(
        u32::try_from(h.types.len())
            .expect("fixture type id fits in u32")
            .into(),
    );
    let function_type = h.function_types.alloc(hir::FunctionType {
        canonical_type: managed_canonical_type,
        is_suspend: false,
        parameter_types: vec![int, int],
        return_type: int,
    });
    assert_eq!(
        h.types.alloc(hir::Type::Function(function_type)),
        managed_canonical_type
    );
    let native_canonical_type = hir::TypeId::from_raw(
        u32::try_from(h.types.len())
            .expect("fixture type id fits in u32")
            .into(),
    );
    let native_function_type = h.function_types.alloc(hir::FunctionType {
        canonical_type: native_canonical_type,
        is_suspend: false,
        parameter_types: vec![int, int, int],
        return_type: int,
    });
    assert_eq!(
        h.types.alloc(hir::Type::Function(native_function_type)),
        native_canonical_type
    );
    let function_pointer = h.types.alloc(hir::Type::FunPtr(native_function_type));
    let context_pointer = h.types.alloc(hir::Type::Ptr(unit));
    let callback = h.strukt(
        "Callback",
        &[("function", function_pointer), ("context", context_pointer)],
    );
    let callback_ty = h.struct_ty(callback);
    let mut target_locals = Arena::new();
    let first = target_locals.alloc(local("first", h.int));
    let second = target_locals.alloc(local("second", h.int));
    let target = h.user_fn_full(
        "callbackTarget",
        Vec::new(),
        vec![param("first", h.int, first), param("second", h.int, second)],
        h.int,
        hir::Body {
            locals: target_locals,
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(local_ref(first, h.int)),
            })],
        },
    );
    let main = empty_main(&mut h);
    let executable = h.finish(main);
    let entry = executable.entry();
    let mut source = executable.into_module();
    let hir::CoreProtocols::Defined(protocols) = &mut source.core_protocols else {
        panic!("test Export HIR carries locally defined core protocols")
    };
    protocols.foreign_callbacks.callback = callback;
    let definition_path = scoop_identity::StructuralDefinitionPath::from_first(
        scoop_identity::StructuralPathSegment::new(
            scoop_identity::StructuralDefinitionSiteRole::CallableConversion,
            0,
        ),
        [],
    );
    let reference = source.callable_references.alloc(hir::CallableReference {
        definition_root: hir::LexicalDefinitionRoot::Function(target),
        definition_path,
        target: hir::CallableReferenceTarget::Named(hir::Callable::Function(target).into()),
        function_type,
        owner_type_arguments: Vec::new(),
        captures: Vec::new(),
        origin: definition_origin(),
        span: SPAN,
    });
    let mode = defined_export_core(&source)
        .foreign_callbacks
        .modes
        .reusable();
    let registration =
        source
            .foreign_callback_registrations
            .alloc(hir::ForeignCallbackRegistration {
                definition_root: hir::LexicalDefinitionRoot::Function(target),
                definition_path: scoop_identity::StructuralDefinitionPath::from_first(
                    scoop_identity::StructuralPathSegment::new(
                        scoop_identity::StructuralDefinitionSiteRole::CallbackConversion,
                        0,
                    ),
                    [],
                ),
                native_function_type,
                managed_function_type: function_type,
                context_index: 0,
                mode,
                span: SPAN,
            });
    let hir::FunctionKind::User(main_body) = &mut source.functions[main].kind else {
        panic!("main is a user function")
    };
    main_body.statements.push(expr_stmt(expr(
        hir::ExprKind::ForeignCallbackRegister {
            registration,
            closure: Box::new(expr(
                hir::ExprKind::CallableReference(reference),
                managed_canonical_type,
            )),
        },
        callback_ty,
    )));
    source.callback_registration_identities = rebuild_callback_identities(&source);
    let expected_application = scoop_identity::PersistentCallbackApplicationId::from_key(
        &scoop_identity::CallbackApplicationKey::new(
            source.callback_registration_identities[registration].key(),
            scoop_identity::CallableMaterializationContext::NoSubstitution,
        )
        .unwrap(),
    )
    .unwrap();

    let source = executable_output(source, entry);
    let concrete =
        scoop_hir_lower::concretize_output(&source).expect("concrete type applications are valid");
    let concrete_reference = concrete
        .module()
        .callable_references
        .iter()
        .next()
        .expect("the callback test has one concrete callable reference")
        .1;
    let reference_materialization = *concrete_reference.identity.materialization();
    let reference_function_type =
        &concrete.module().function_types[concrete_reference.function_type];
    let reference_signature = scoop_identity::ExactCallableSignature::new(
        scoop_identity::Effect::Ordinary,
        None,
        reference_function_type
            .parameter_types
            .iter()
            .map(|parameter| concrete.module().exact_type_identities[*parameter].id())
            .collect(),
        concrete.module().exact_type_identities[reference_function_type.return_type].id(),
    );
    let callback_core = defined_concrete_core(concrete.module()).foreign_callbacks;
    let expected_callback_protocol_exact_types = [
        callback_core.modes.enumeration(),
        callback_core.states.enumeration(),
        callback_core.failure_result.enumeration(),
    ]
    .map(|enumeration| {
        concrete.module().exact_type_identities[concrete.module().enums[enumeration].canonical_type]
            .id()
    });
    let module = crate::lower(&concrete)
        .expect("test LocalConcrete HIR carries locally defined core protocols");
    let reference_invoke = module
        .functions
        .iter()
        .find_map(|(function, definition)| {
            definition
                .name
                .starts_with("$reference.")
                .then_some(function)
        })
        .expect("the callable reference has one invoke wrapper");
    let reference_source = module
        .meta
        .source_callable_materializations
        .get(reference_invoke)
        .expect("the callable-reference wrapper has an exact MIR location");
    assert_eq!(
        reference_source.materialization(),
        reference_materialization
    );
    assert_eq!(
        reference_source.signature_record().signature(),
        &reference_signature
    );
    assert_eq!(
        module.functions[reference_invoke].params.len(),
        reference_signature.parameters().len() + 1,
        "the closure environment is physical and does not enter the Scoop signature"
    );
    let (_, bridge) = module
        .foreign_callback_bridges
        .iter()
        .next()
        .expect("registration generates one native bridge");
    assert_eq!(bridge.application(), expected_application);
    let family = module.foreign_callback_families[bridge.family];
    assert_eq!(family.callback, module.structs.iter().next().unwrap().0);
    let callback_protocol_types = [
        family.modes.enum_id(),
        family.states.enum_id(),
        family.failure_result.enum_id(),
    ]
    .map(|enumeration| {
        mir::Type::Enum(
            enumeration,
            module.enums[enumeration].type_arguments.clone(),
        )
    });
    for (ty, expected) in callback_protocol_types
        .iter()
        .zip(expected_callback_protocol_exact_types)
    {
        assert_eq!(
            module
                .meta
                .source_exact_types
                .get(ty)
                .expect("foreign callback protocol type crosses into MIR")
                .identity_record()
                .id(),
            expected
        );
    }
    assert_eq!(
        module.enums[family.states.enum_id()].name,
        "ForeignCallbackState"
    );
    assert_eq!(module.enums[family.failure_result.enum_id()].name, "Option");
    assert_eq!(
        family
            .modes
            .reusable()
            .definition(&module.enums)
            .unwrap()
            .name,
        "Reusable"
    );
    let (_, adapter) = module
        .foreign_callback_adapters
        .iter()
        .next()
        .expect("registration generates one managed adapter");
    assert!(matches!(
        adapter.identity_record().key(),
        scoop_identity::GeneratedCallableKey::ForeignCallbackManagedAdapter { application }
            if *application == expected_application
    ));
    assert!(matches!(
        adapter.signature_subject(),
        mir::CallableSignatureSubject::Strong(scoop_identity::CallableOwner::Generated(generated))
            if generated == adapter.identity_record().id()
    ));
    let generated = module
        .meta
        .generated_callables
        .get(adapter.function)
        .expect("the managed adapter has one generated callable location");
    assert_eq!(generated.identity_record(), adapter.identity_record());
    assert_eq!(
        bridge.application_identity.id(),
        bridge.application_record.application()
    );
    assert_eq!(
        bridge
            .application_record
            .managed_signature()
            .parameters()
            .len(),
        2
    );
    assert_eq!(
        bridge.application_record.mode(),
        scoop_identity::CallbackMode::Reusable
    );
    let function = &module.functions[adapter.function];
    assert_eq!(
        function.return_ty,
        mir::Type::MachineScalar(mir::MachineScalarKind::ForeignCallbackStatus)
    );
    assert_callback_status(
        &function.body.blocks[function.body.entry].terminator,
        mir::ForeignCallbackStatus::Returned,
    );
    let catch = function.body.blocks[function.body.entry]
        .unwind
        .expect("the adapter catches managed exceptions");
    assert_callback_status(
        &function.body.blocks[catch].terminator,
        mir::ForeignCallbackStatus::Threw,
    );

    let (call, _) = statement_call(&function.body.blocks[function.body.entry].statements[0]);
    let offsets = call.args[1..]
        .iter()
        .map(callback_argument_offset)
        .collect::<Vec<_>>();
    assert_eq!(offsets, [0, 1]);

    let foundation = assert_mir_foundation_projection(&module);
    assert_eq!(foundation.callback_applications, 1);
    assert_eq!(foundation.callback_application_records, 1);

    let dump = mir::dump(&module);
    assert!(dump.contains("-> machine<foreign-callback-status>"));
    assert!(dump.contains("MachineScalarLiteral ForeignCallbackStatus(Returned)"));
    assert!(dump.contains("MachineScalarLiteral ForeignCallbackStatus(Threw)"));
    assert!(dump.contains("MachineScalarLiteral PointerElementOffset(0)"));
    assert!(dump.contains("MachineScalarLiteral PointerElementOffset(1)"));
}

fn assert_callback_status(terminator: &mir::Terminator, expected: mir::ForeignCallbackStatus) {
    let mir::Terminator::Return { value: Some(value) } = terminator else {
        panic!("callback adapter exits with a status")
    };
    assert_eq!(
        value.ty,
        mir::Type::MachineScalar(mir::MachineScalarKind::ForeignCallbackStatus)
    );
    assert!(matches!(
        value.kind,
        mir::ExprKind::MachineScalarLiteral(
            mir::MachineScalarValue::ForeignCallbackStatus(status)
        ) if status == expected
    ));
}

fn callback_argument_offset(argument: &mir::Expr) -> u64 {
    let mir::ExprKind::PtrLoad {
        pointer,
        offset: None,
        ..
    } = &argument.kind
    else {
        panic!("managed callback argument is loaded through its typed pointer")
    };
    let mir::ExprKind::PtrCast { operand, .. } = &pointer.kind else {
        panic!("the opaque callback argument pointer is cast to its pointee type")
    };
    let mir::ExprKind::PtrLoad {
        offset: Some(offset),
        ..
    } = &operand.kind
    else {
        panic!("the callback argument slot load carries an element offset")
    };
    assert_eq!(
        offset.ty,
        mir::Type::MachineScalar(mir::MachineScalarKind::PointerElementOffset)
    );
    let mir::ExprKind::MachineScalarLiteral(mir::MachineScalarValue::PointerElementOffset(offset)) =
        offset.kind
    else {
        panic!("callback argument offsets are compiler-owned machine scalars")
    };
    offset
}
