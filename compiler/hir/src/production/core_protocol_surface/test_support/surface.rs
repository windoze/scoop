use super::*;

pub(super) fn install_at(
    foundation: &mut CanonicalHirFoundation,
    existing: ExistingProtocolFixture,
    origin: ConeIdentity,
) -> CoreCompilerProtocolSurfaceV1 {
    install_with_intrinsics_at(foundation, existing, origin).0
}

pub(super) fn install_with_intrinsics_at(
    foundation: &mut CanonicalHirFoundation,
    existing: ExistingProtocolFixture,
    origin: ConeIdentity,
) -> (
    CoreCompilerProtocolSurfaceV1,
    Vec<(IntrinsicFunctionKind, CoreProtocolCallableV1)>,
) {
    let mut builder = FixtureBuilder::new(existing, origin);

    let integers: [PersistentTypeId; 8] =
        std::array::from_fn(|_| builder.concrete_nominal(SourceNominalKind::Struct));
    let boolean = builder.concrete_nominal(SourceNominalKind::Struct);
    let array = builder.generic_nominal(SourceNominalKind::Class);
    let mutable_array = builder.generic_nominal(SourceNominalKind::Class);
    let ptr = builder.generic_nominal(SourceNominalKind::Struct);
    let fun_ptr = builder.generic_nominal(SourceNominalKind::Struct);
    let fundamental_types = CoreFundamentalTypeProtocolV1 {
        unit: CoreBuiltinNominal::Unit.identity_record().id(),
        int8: integers[0],
        int16: integers[1],
        int32: integers[2],
        int64: integers[3],
        uint8: integers[4],
        uint16: integers[5],
        uint32: integers[6],
        uint64: integers[7],
        boolean,
        string: builder.existing.string.id(),
        array,
        mutable_array,
        ptr,
        fun_ptr,
        character: builder.concrete_nominal(SourceNominalKind::Struct),
        float: builder.concrete_nominal(SourceNominalKind::Struct),
        double: builder.concrete_nominal(SourceNominalKind::Struct),
        any: builder.concrete_nominal(SourceNominalKind::Class),
        nothing: builder.concrete_nominal(SourceNominalKind::Class),
    };

    let option_protocol = CoreOptionProtocolV1 {
        option: builder.existing.option.id(),
        some: builder.existing.option_some.id(),
        some_payload: builder.existing.option_some_payload.id(),
        none: builder.existing.option_none.id(),
    };
    let option_type = builder.existing.option.id();
    let string_type = builder.existing.string.id();

    let iterator = builder.generic_nominal(SourceNominalKind::Interface);
    let iterator_id = iterator;
    let (next, next_id) = builder.function_with_owner_signature(
        Some(DefinitionOwnerAtom::GenericType(iterator_id)),
        SignatureCallableShape::new(
            scoop_identity::Effect::Ordinary,
            None,
            Vec::new(),
            signature_application(option_type, SignatureTypeKey::Binder { depth: 0, index: 0 }),
        ),
    );
    let iteration_protocol = CoreIterationProtocolV1 {
        iterator,
        next,
        next_dispatch: builder.dispatch(next_id),
    };

    let exception_classes: [PersistentTypeId; 8] = std::array::from_fn(|_| builder.concrete_type());
    let exception_constructors = std::array::from_fn::<_, 8, _>(|index| {
        let parameters = if index == 7 {
            vec![signature_application(
                option_type,
                SignatureTypeKey::Nominal(string_type),
            )]
        } else {
            Vec::new()
        };
        builder.constructor(exception_classes[index], parameters)
    });
    let initialization_cycle_thrower = builder
        .function_with_owner_signature(
            None,
            SignatureCallableShape::new(
                scoop_identity::Effect::Ordinary,
                None,
                vec![SignatureTypeKey::Nominal(string_type)],
                SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
            ),
        )
        .0;
    let exception_protocol = CoreExceptionProtocolV1 {
        throwable: exception_classes[0],
        throwable_constructor: exception_constructors[0].clone(),
        unwrap_exception: exception_classes[1],
        unwrap_exception_constructor: exception_constructors[1].clone(),
        class_cast_exception: exception_classes[2],
        class_cast_exception_constructor: exception_constructors[2].clone(),
        arithmetic_exception: exception_classes[3],
        arithmetic_exception_constructor: exception_constructors[3].clone(),
        index_out_of_bounds_exception: exception_classes[4],
        index_out_of_bounds_exception_constructor: exception_constructors[4].clone(),
        illegal_state_exception: exception_classes[5],
        illegal_state_exception_constructor: exception_constructors[5].clone(),
        initialization_cycle_thrower,
        illegal_argument_exception: exception_classes[6],
        illegal_argument_exception_constructor: exception_constructors[6].clone(),
        missing_context_exception: exception_classes[7],
        missing_context_exception_constructor: exception_constructors[7].clone(),
    };

    let continuation = builder.generic_nominal(SourceNominalKind::Interface);
    let continuation_id = continuation;
    let (resume, resume_id) = builder.function_with_owner_signature(
        Some(DefinitionOwnerAtom::GenericType(continuation_id)),
        SignatureCallableShape::new(
            scoop_identity::Effect::Ordinary,
            None,
            vec![SignatureTypeKey::Binder { depth: 0, index: 0 }],
            SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
        ),
    );
    let (resume_exception, resume_exception_id) = builder.function_with_owner_signature(
        Some(DefinitionOwnerAtom::GenericType(continuation_id)),
        SignatureCallableShape::new(
            scoop_identity::Effect::Ordinary,
            None,
            vec![SignatureTypeKey::Nominal(exception_classes[0])],
            SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
        ),
    );
    let suspend_task = builder.generic_nominal(SourceNominalKind::Interface);
    let (run, run_id) = builder.function_with_owner_signature(
        Some(DefinitionOwnerAtom::GenericType(suspend_task)),
        SignatureCallableShape::new(
            scoop_identity::Effect::Suspend,
            None,
            Vec::new(),
            SignatureTypeKey::Binder { depth: 0, index: 0 },
        ),
    );
    let suspend_registration = builder.generic_nominal(SourceNominalKind::Interface);
    let (register, register_id) = builder.function_with_owner_signature(
        Some(DefinitionOwnerAtom::GenericType(suspend_registration)),
        SignatureCallableShape::new(
            scoop_identity::Effect::Ordinary,
            None,
            vec![signature_application(
                continuation_id,
                SignatureTypeKey::Binder { depth: 0, index: 0 },
            )],
            SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
        ),
    );
    let (start, _) = builder.function(scoop_identity::Effect::Ordinary);
    let (suspend, _) = builder.function(scoop_identity::Effect::Suspend);
    let mut coroutine_protocol = CoreCoroutineProtocolV1 {
        continuation,
        continuation_resume: resume,
        continuation_resume_dispatch: builder.dispatch(resume_id),
        continuation_resume_with_exception: resume_exception,
        continuation_resume_with_exception_dispatch: builder.dispatch(resume_exception_id),
        suspend_task,
        suspend_task_run: run,
        suspend_task_run_dispatch: builder.dispatch(run_id),
        suspend_registration,
        suspend_registration_register: register,
        suspend_registration_register_dispatch: builder.dispatch(register_id),
        start_coroutine: start,
        suspend_coroutine: suspend,
    };

    let pinned = builder.generic_nominal(SourceNominalKind::Struct);
    let handle = builder.generic_nominal(SourceNominalKind::Struct);
    let ffi_callables: [CoreProtocolCallableV1; 15] =
        std::array::from_fn(|_| builder.function(scoop_identity::Effect::Ordinary).0);
    let mut ffi_protocol = CoreFfiProtocolV1 {
        ptr,
        fun_ptr,
        pinned_ptr: pinned,
        gc_handle: handle,
        ptr_to_ulong: ffi_callables[0].clone(),
        ptr_cast: ffi_callables[1].clone(),
        ptr_load: ffi_callables[2].clone(),
        ptr_load_offset: ffi_callables[3].clone(),
        ptr_store: ffi_callables[4].clone(),
        ptr_store_offset: ffi_callables[5].clone(),
        ptr_plus: ffi_callables[6].clone(),
        ptr_minus: ffi_callables[7].clone(),
        address_of: ffi_callables[8].clone(),
        size_of: ffi_callables[9].clone(),
        align_of: ffi_callables[10].clone(),
        gc_pin_raw: ffi_callables[11].clone(),
        gc_unpin_raw: ffi_callables[12].clone(),
        gc_get_handle_raw: ffi_callables[13].clone(),
        gc_release_handle_raw: ffi_callables[14].clone(),
    };

    let callback = builder.generic_nominal(SourceNominalKind::Struct);
    let mode = builder.concrete_enum_with_variants(2);
    let state = builder.concrete_enum_with_variants(4);
    let option_id = builder.existing.option.id();
    let failure_exact = builder.exact_application(option_id, exception_classes[0]);
    let callback_callables: [CoreProtocolCallableV1; 5] =
        std::array::from_fn(|_| builder.function(scoop_identity::Effect::Ordinary).0);
    let mut foreign_callback_protocol = CoreForeignCallbackProtocolV1 {
        callback,
        mode: mode.owner,
        reusable: mode.variants[0],
        one_shot: mode.variants[1],
        state: state.owner,
        registered: state.variants[0],
        active: state.variants[1],
        completed: state.variants[2],
        failed: state.variants[3],
        failure_result: failure_exact,
        register: callback_callables[0].clone(),
        retain: callback_callables[1].clone(),
        release: callback_callables[2].clone(),
        query_state: callback_callables[3].clone(),
        failure: callback_callables[4].clone(),
    };

    let source_location = builder.concrete_nominal(SourceNominalKind::Struct);
    let current = builder.function(scoop_identity::Effect::Ordinary).0;
    let mut source_location_protocol = CoreSourceLocationProtocolV1 {
        location: source_location,
        current,
    };

    let program_arguments = builder
        .function_with_owner_signature(
            None,
            SignatureCallableShape::new(
                scoop_identity::Effect::Ordinary,
                None,
                Vec::new(),
                SignatureTypeKey::NominalApplication {
                    origin: fundamental_types.array,
                    arguments: scoop_identity::NonEmptyVec::new(vec![SignatureTypeKey::Nominal(
                        string_type,
                    )])
                    .unwrap(),
                },
            ),
        )
        .0;
    let type_reference = builder.operation(
        None,
        1,
        SignatureCallableShape::new(
            scoop_identity::Effect::Ordinary,
            None,
            Vec::new(),
            SignatureTypeKey::Nominal(source_location),
        ),
    );
    let signature_surface = CoreCompilerProtocolSurfaceV1 {
        program_arguments: program_arguments.clone(),
        type_reference: type_reference.clone(),
        fundamental_types: fundamental_types.clone(),
        option_protocol: option_protocol.clone(),
        iteration_protocol: iteration_protocol.clone(),
        exception_protocol: exception_protocol.clone(),
        coroutine_protocol: coroutine_protocol.clone(),
        ffi_protocol: ffi_protocol.clone(),
        foreign_callback_protocol: foreign_callback_protocol.clone(),
        source_location_protocol: source_location_protocol.clone(),
    };
    let operations = intrinsic_function_kinds()
        .into_iter()
        .filter(|kind| {
            !matches!(
                kind,
                IntrinsicFunctionKind::Atomic(_)
                    | IntrinsicFunctionKind::MaybeUninit(_)
                    | IntrinsicFunctionKind::TypeReference
                    | IntrinsicFunctionKind::TypeDescription
                    | IntrinsicFunctionKind::AnnotationKey
            )
        })
        .map(|kind| {
            let callable = builder.operation(
                expected_operation_owner(&signature_surface, kind),
                operation_own_type_parameter_count(kind),
                expected_operation_signature(&signature_surface, kind),
            );
            (kind, callable)
        })
        .collect::<Vec<_>>();

    coroutine_protocol.start_coroutine =
        operation_entry(&operations, IntrinsicFunctionKind::CoroutineStart);
    coroutine_protocol.suspend_coroutine =
        operation_entry(&operations, IntrinsicFunctionKind::CoroutineSuspend);
    for (callable, kind) in [
        (
            &mut ffi_protocol.ptr_to_ulong,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::ToULong),
        ),
        (
            &mut ffi_protocol.ptr_cast,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Cast),
        ),
        (
            &mut ffi_protocol.ptr_load,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Load),
        ),
        (
            &mut ffi_protocol.ptr_load_offset,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::LoadOffset),
        ),
        (
            &mut ffi_protocol.ptr_store,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Store),
        ),
        (
            &mut ffi_protocol.ptr_store_offset,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::StoreOffset),
        ),
        (
            &mut ffi_protocol.ptr_plus,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Plus),
        ),
        (
            &mut ffi_protocol.ptr_minus,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Minus),
        ),
        (
            &mut ffi_protocol.address_of,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::AddressOf),
        ),
        (
            &mut ffi_protocol.size_of,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::SizeOf),
        ),
        (
            &mut ffi_protocol.align_of,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::AlignOf),
        ),
        (
            &mut ffi_protocol.gc_pin_raw,
            IntrinsicFunctionKind::GcPinRaw,
        ),
        (
            &mut ffi_protocol.gc_unpin_raw,
            IntrinsicFunctionKind::GcUnpinRaw,
        ),
        (
            &mut ffi_protocol.gc_get_handle_raw,
            IntrinsicFunctionKind::GcGetHandleRaw,
        ),
        (
            &mut ffi_protocol.gc_release_handle_raw,
            IntrinsicFunctionKind::GcReleaseHandleRaw,
        ),
    ] {
        *callable = operation_entry(&operations, kind);
    }
    for (callable, kind) in [
        (
            &mut foreign_callback_protocol.register,
            IntrinsicFunctionKind::ForeignCallbackRegister,
        ),
        (
            &mut foreign_callback_protocol.retain,
            IntrinsicFunctionKind::ForeignCallbackRetain,
        ),
        (
            &mut foreign_callback_protocol.release,
            IntrinsicFunctionKind::ForeignCallbackRelease,
        ),
        (
            &mut foreign_callback_protocol.query_state,
            IntrinsicFunctionKind::ForeignCallbackState,
        ),
        (
            &mut foreign_callback_protocol.failure,
            IntrinsicFunctionKind::ForeignCallbackFailure,
        ),
    ] {
        *callable = operation_entry(&operations, kind);
    }
    source_location_protocol.current =
        operation_entry(&operations, IntrinsicFunctionKind::CurrentSourceLocation);

    let surface = CoreCompilerProtocolSurfaceV1 {
        program_arguments,
        type_reference,
        fundamental_types,
        option_protocol,
        iteration_protocol,
        exception_protocol,
        coroutine_protocol,
        ffi_protocol,
        foreign_callback_protocol,
        source_location_protocol,
    };
    surface.validate_internal_relations().unwrap();
    builder.install(foundation);
    (surface, operations)
}
