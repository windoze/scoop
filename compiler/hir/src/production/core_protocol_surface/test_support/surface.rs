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

    let integers: [CoreProtocolEntryV1; 8] =
        std::array::from_fn(|_| builder.concrete_nominal(SourceNominalKind::Struct));
    let boolean = builder.concrete_nominal(SourceNominalKind::Struct);
    let array = builder.generic_nominal(SourceNominalKind::Class);
    let mutable_array = builder.generic_nominal(SourceNominalKind::Class);
    let ptr = builder.generic_nominal(SourceNominalKind::Struct);
    let fun_ptr = builder.generic_nominal(SourceNominalKind::Struct);
    let fundamental_types = CoreFundamentalTypeProtocolV1(product([
        concrete_entry(CoreBuiltinNominal::Unit.identity_record().id()),
        integers[0].clone(),
        integers[1].clone(),
        integers[2].clone(),
        integers[3].clone(),
        integers[4].clone(),
        integers[5].clone(),
        integers[6].clone(),
        integers[7].clone(),
        boolean,
        concrete_entry(builder.existing.string.id()),
        array,
        mutable_array,
        ptr.clone(),
        fun_ptr.clone(),
        builder.concrete_nominal(SourceNominalKind::Struct),
        builder.concrete_nominal(SourceNominalKind::Struct),
        builder.concrete_nominal(SourceNominalKind::Struct),
    ]));

    let option_protocol = CoreOptionProtocolV1(product([
        generic_entry(builder.existing.option.id()),
        CoreProtocolEntryV1::EnumVariant(builder.existing.option_some.id()),
        CoreProtocolEntryV1::EnumVariantField(builder.existing.option_some_payload.id()),
        CoreProtocolEntryV1::EnumVariant(builder.existing.option_none.id()),
    ]));
    let option_type = builder.existing.option.id();
    let string_type = builder.existing.string.id();

    let iterator = builder.generic_nominal(SourceNominalKind::Interface);
    let iterator_id = match iterator {
        CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(id)) => id,
        _ => unreachable!("the fixture iterator is generic"),
    };
    let (next, next_id) = builder.function_with_owner_signature(
        Some(DefinitionOwnerAtom::GenericType(iterator_id)),
        SignatureCallableShape::new(
            scoop_identity::Effect::Ordinary,
            None,
            Vec::new(),
            signature_application(option_type, SignatureTypeKey::Binder { depth: 0, index: 0 }),
        ),
    );
    let iteration_protocol =
        CoreIterationProtocolV1(product([iterator, next, builder.dispatch(next_id)]));

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
    let exception_protocol = CoreExceptionProtocolV1(product([
        concrete_entry(exception_classes[0]),
        exception_constructors[0].clone(),
        concrete_entry(exception_classes[1]),
        exception_constructors[1].clone(),
        concrete_entry(exception_classes[2]),
        exception_constructors[2].clone(),
        concrete_entry(exception_classes[3]),
        exception_constructors[3].clone(),
        concrete_entry(exception_classes[4]),
        exception_constructors[4].clone(),
        concrete_entry(exception_classes[5]),
        exception_constructors[5].clone(),
        initialization_cycle_thrower,
        concrete_entry(exception_classes[6]),
        exception_constructors[6].clone(),
        concrete_entry(exception_classes[7]),
        exception_constructors[7].clone(),
    ]));

    let continuation = builder.generic_nominal(SourceNominalKind::Interface);
    let continuation_id = generic_protocol_id(&continuation);
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
        Some(DefinitionOwnerAtom::GenericType(generic_protocol_id(
            &suspend_task,
        ))),
        SignatureCallableShape::new(
            scoop_identity::Effect::Suspend,
            None,
            Vec::new(),
            SignatureTypeKey::Binder { depth: 0, index: 0 },
        ),
    );
    let suspend_registration = builder.generic_nominal(SourceNominalKind::Interface);
    let (register, register_id) = builder.function_with_owner_signature(
        Some(DefinitionOwnerAtom::GenericType(generic_protocol_id(
            &suspend_registration,
        ))),
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
    let mut coroutine_protocol = CoreCoroutineProtocolV1(product([
        continuation,
        resume,
        builder.dispatch(resume_id),
        resume_exception,
        builder.dispatch(resume_exception_id),
        suspend_task,
        run,
        builder.dispatch(run_id),
        suspend_registration,
        register,
        builder.dispatch(register_id),
        start,
        suspend,
    ]));

    let pinned = builder.generic_nominal(SourceNominalKind::Struct);
    let handle = builder.generic_nominal(SourceNominalKind::Struct);
    let ffi_callables: [CoreProtocolEntryV1; 15] =
        std::array::from_fn(|_| builder.function(scoop_identity::Effect::Ordinary).0);
    let mut ffi_protocol = CoreFfiProtocolV1(product([
        ptr,
        fun_ptr,
        pinned,
        handle,
        ffi_callables[0].clone(),
        ffi_callables[1].clone(),
        ffi_callables[2].clone(),
        ffi_callables[3].clone(),
        ffi_callables[4].clone(),
        ffi_callables[5].clone(),
        ffi_callables[6].clone(),
        ffi_callables[7].clone(),
        ffi_callables[8].clone(),
        ffi_callables[9].clone(),
        ffi_callables[10].clone(),
        ffi_callables[11].clone(),
        ffi_callables[12].clone(),
        ffi_callables[13].clone(),
        ffi_callables[14].clone(),
    ]));

    let callback = builder.generic_nominal(SourceNominalKind::Struct);
    let mode = builder.concrete_enum_with_variants(2);
    let state = builder.concrete_enum_with_variants(4);
    let option_id = builder.existing.option.id();
    let failure_exact = builder.exact_application(option_id, exception_classes[0]);
    let callback_callables: [CoreProtocolEntryV1; 5] =
        std::array::from_fn(|_| builder.function(scoop_identity::Effect::Ordinary).0);
    let mut foreign_callback_protocol = CoreForeignCallbackProtocolV1(product([
        callback,
        concrete_entry(mode.owner),
        CoreProtocolEntryV1::EnumVariant(mode.variants[0]),
        CoreProtocolEntryV1::EnumVariant(mode.variants[1]),
        concrete_entry(state.owner),
        CoreProtocolEntryV1::EnumVariant(state.variants[0]),
        CoreProtocolEntryV1::EnumVariant(state.variants[1]),
        CoreProtocolEntryV1::EnumVariant(state.variants[2]),
        CoreProtocolEntryV1::EnumVariant(state.variants[3]),
        CoreProtocolEntryV1::ExactType(failure_exact),
        callback_callables[0].clone(),
        callback_callables[1].clone(),
        callback_callables[2].clone(),
        callback_callables[3].clone(),
        callback_callables[4].clone(),
    ]));

    let source_location = builder.concrete_nominal(SourceNominalKind::Struct);
    let current = builder.function(scoop_identity::Effect::Ordinary).0;
    let mut source_location_protocol =
        CoreSourceLocationProtocolV1(product([source_location, current]));

    let signature_surface = CoreCompilerProtocolSurfaceV1 {
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
        .map(|kind| {
            let callable = builder.operation(
                fixture_operation_owner(kind, &fundamental_types),
                operation_own_type_parameter_count(kind),
                expected_operation_signature(&signature_surface, kind),
            );
            (kind, callable_entry(callable))
        })
        .collect::<Vec<_>>();

    coroutine_protocol.0.entries[11] =
        operation_entry(&operations, IntrinsicFunctionKind::CoroutineStart);
    coroutine_protocol.0.entries[12] =
        operation_entry(&operations, IntrinsicFunctionKind::CoroutineSuspend);
    for (index, kind) in [
        (
            4,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::ToULong),
        ),
        (
            5,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Cast),
        ),
        (
            6,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Load),
        ),
        (
            7,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::LoadOffset),
        ),
        (
            8,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Store),
        ),
        (
            9,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::StoreOffset),
        ),
        (
            10,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Plus),
        ),
        (
            11,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Minus),
        ),
        (
            12,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::AddressOf),
        ),
        (
            13,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::SizeOf),
        ),
        (
            14,
            IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::AlignOf),
        ),
        (15, IntrinsicFunctionKind::GcPinRaw),
        (16, IntrinsicFunctionKind::GcUnpinRaw),
        (17, IntrinsicFunctionKind::GcGetHandleRaw),
        (18, IntrinsicFunctionKind::GcReleaseHandleRaw),
    ] {
        ffi_protocol.0.entries[index] = operation_entry(&operations, kind);
    }
    for (index, kind) in [
        (10, IntrinsicFunctionKind::ForeignCallbackRegister),
        (11, IntrinsicFunctionKind::ForeignCallbackRetain),
        (12, IntrinsicFunctionKind::ForeignCallbackRelease),
        (13, IntrinsicFunctionKind::ForeignCallbackState),
        (14, IntrinsicFunctionKind::ForeignCallbackFailure),
    ] {
        foreign_callback_protocol.0.entries[index] = operation_entry(&operations, kind);
    }
    source_location_protocol.0.entries[1] =
        operation_entry(&operations, IntrinsicFunctionKind::CurrentSourceLocation);

    let surface = CoreCompilerProtocolSurfaceV1 {
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
