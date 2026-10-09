use super::{
    AbiArgument, AbiCallArgument, AbiNonZeroLayout, AbiReturn, AbiValue, AbiZeroSizedLayout,
    AbiZst, CCallDestination, CExternFunction, CExternFunctionRef, CallDestination, CallTarget,
    CallTargets, CallingConvention, CoroutineAdapterState, CoroutineFrameState,
    CoroutineSuspendStateId, DirectCallSignature, ElidedZstCallSignature, EnumDef, EnumDefs,
    EnumFieldRepr, EnumRepr, EnumVariantRepr, ExternFunctionIdentity, ExternFunctions,
    ForeignCallbackFailureResult, ForeignCallbackModes, ForeignCallbackStates,
    ForeignCallbackStatus, GcEffect, IndirectResultCallSignature, IndirectResultConvention,
    InitializationOutcome, InternalPointerCarrier, LirTargetProfile, LirType,
    LocalFunctionIdentities, MachineScalarKind, MachineScalarValue, ManagedCallDestination,
    ManagedRuntimeFunction, NativeBorrowedCallDestination, NativeBorrowedResultPublication,
    NativeBorrowedResultRoot, NonEmptyRefScan, NullNicheKind, PointerKind, PointerNullEncoding,
    RefScan, ScoopAbiSignature, ScoopExternFunction, ScoopExternFunctionRef, TargetProfileId,
    TypedCall, TypedCallResult, TypedCallView, Value, VoidCallSignature,
};

pub(crate) fn test_physical_exact(
    name: &str,
    kind: scoop_identity::SourceNominalKind,
) -> scoop_identity::PersistentExactTypeId {
    use scoop_identity::{
        CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, ExactTypeKey,
        PackagePath, PersistentExactTypeId, PersistentTypeId, SourceDeclarationKey,
        SourceDeclarationSite,
    };
    let identifier = format!(
        "test{}",
        name.bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let declaration = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(&identifier).unwrap(),
        kind,
        0,
    );
    let nominal = PersistentTypeId::from_source_declaration(&declaration).unwrap();
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap()
}

fn abi_value(ty: LirType, size: u64, alignment: u64, scan: RefScan) -> AbiValue {
    AbiValue::new(
        ty,
        AbiNonZeroLayout::new(size, alignment).expect("test ABI layout must be valid"),
        scan,
    )
    .expect("test ABI value must be valid")
}

pub(crate) fn callable_body(symbol: &str) -> super::CallableBodyIdentity {
    let identifier = format!(
        "test{}",
        symbol
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let site = scoop_identity::SourceDeclarationSite::new(
        scoop_identity::ConeIdentity::SINGLE_FILE,
        scoop_identity::PackagePath::root(),
        scoop_identity::DefinitionOwnerChain::top_level(),
        scoop_identity::DeclarationScope::ConeWide,
    )
    .unwrap();
    let declaration = scoop_identity::SourceDeclarationKey::function(
        site,
        scoop_identity::CanonicalIdentifier::new(&identifier).unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function =
        scoop_identity::PersistentFunctionId::from_source_declaration(&declaration).unwrap();
    super::CallableBodyIdentity::for_function(function).unwrap()
}

fn abi_zst(ty: LirType, alignment: u64) -> AbiZst {
    AbiZst::new(
        ty,
        AbiZeroSizedLayout::new(alignment).expect("test ZST layout must be valid"),
    )
    .expect("test ABI ZST must be valid")
}

#[test]
fn darwin_aarch64_profile_fixes_every_backend_scalar_layout() {
    let profile = LirTargetProfile::DARWIN_AARCH64;

    assert_eq!(profile.id(), TargetProfileId::DarwinAarch64);
    assert_eq!(profile.id().canonical_name(), "darwin-aarch64");
    assert_eq!(
        profile.canonical_llvm_data_layout(),
        "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
    );

    for (kind, size, alignment) in [
        (super::BackendScalarKind::I1, 1, 1),
        (super::BackendScalarKind::I8, 1, 1),
        (super::BackendScalarKind::I16, 2, 2),
        (super::BackendScalarKind::I32, 4, 4),
        (super::BackendScalarKind::I64, 8, 8),
    ] {
        let layout = profile.scalar_layout(kind);
        assert_eq!(layout.size_bytes(), size);
        assert_eq!(layout.alignment_bytes(), alignment);
    }
}

#[test]
fn foreign_callback_role_bundles_lock_wire_ordinals_and_failure_provenance() {
    let unit_variant = || EnumVariantRepr {
        fields: Vec::new(),
        slot_offset: 8,
        slot_size: 0,
        slot_align: 1,
        gc_free: true,
    };
    let mut enums = EnumDefs::default();
    let mode = enums.alloc(EnumDef {
        exact_type: crate::tests::test_physical_exact(
            "ForeignCallbackMode",
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: "ForeignCallbackMode".to_string(),
        repr: EnumRepr::Tagged {
            variants: vec![unit_variant(), unit_variant()],
            size: 8,
            align: 8,
        },
        scan: RefScan::None,
    });
    let state = enums.alloc(EnumDef {
        exact_type: crate::tests::test_physical_exact(
            "ForeignCallbackState",
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: "ForeignCallbackState".to_string(),
        repr: EnumRepr::Tagged {
            variants: (0..4).map(|_| unit_variant()).collect(),
            size: 8,
            align: 8,
        },
        scan: RefScan::None,
    });
    let failure = enums.alloc(EnumDef {
        exact_type: crate::tests::test_physical_exact(
            "Option<Throwable>",
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: "Option<Throwable>".to_string(),
        repr: EnumRepr::Niche {
            kind: NullNicheKind::Managed,
            payload_variant: 0,
        },
        scan: RefScan::References(vec![0]),
    });
    let reusable = enums.variant_ref(mode, 0).unwrap();
    let one_shot = enums.variant_ref(mode, 1).unwrap();
    let modes = ForeignCallbackModes::checked(&enums, reusable, one_shot).unwrap();
    assert_eq!(modes.runtime_code(modes.reusable()), Some(0));
    assert_eq!(modes.runtime_code(modes.one_shot()), Some(1));
    assert!(ForeignCallbackModes::checked(&enums, one_shot, reusable).is_none());

    let states = ForeignCallbackStates::checked(
        &enums,
        enums.variant_ref(state, 0).unwrap(),
        enums.variant_ref(state, 1).unwrap(),
        enums.variant_ref(state, 2).unwrap(),
        enums.variant_ref(state, 3).unwrap(),
    )
    .unwrap();
    assert!(
        ForeignCallbackStates::checked(
            &enums,
            states.active(),
            states.registered(),
            states.completed(),
            states.failed(),
        )
        .is_none()
    );
    let some = enums.variant_ref(failure, 0).unwrap();
    assert!(
        ForeignCallbackFailureResult::checked(
            &enums,
            enums.variant_field_ref(some, 0).unwrap(),
            enums.variant_ref(failure, 1).unwrap(),
        )
        .is_some()
    );
}

#[test]
fn darwin_aarch64_profile_keeps_pointer_provenance_and_qualification_typed() {
    let profile = LirTargetProfile::DARWIN_AARCH64;
    let word = profile.scalar_layout(super::BackendScalarKind::I64);

    assert_eq!(profile.managed_pointer_layout(), word);
    assert_eq!(profile.metadata_pointer_layout(), word);
    for kind in [
        PointerKind::Managed,
        PointerKind::Raw,
        PointerKind::Code,
        PointerKind::Metadata,
    ] {
        assert_eq!(profile.pointer_layout(kind), word);
    }

    for representation in [profile.data_pointer(), profile.code_pointer()] {
        assert_eq!(representation.layout(), word);
        assert_eq!(
            representation.null_encoding(),
            PointerNullEncoding::AllZeroBits
        );
        assert_eq!(
            representation.carrier(),
            InternalPointerCarrier::BitPreservingU64
        );
    }
}

#[test]
fn niche_representation_atomically_preserves_source_pointer_provenance() {
    for (kind, expected) in [
        (NullNicheKind::Managed, PointerKind::Managed),
        (NullNicheKind::Raw, PointerKind::Raw),
        (NullNicheKind::Code, PointerKind::Code),
    ] {
        let repr = EnumRepr::Niche {
            kind,
            payload_variant: 1,
        };
        let EnumRepr::Niche {
            kind,
            payload_variant,
        } = repr
        else {
            panic!("the test constructs a niche representation");
        };

        assert_eq!(kind.storage_type(), LirType::Ptr(expected));
        assert_eq!(payload_variant, 1);
    }
}

#[test]
fn enum_store_is_the_only_checked_variant_and_payload_field_ref_producer() {
    let mut enums = EnumDefs::default();
    let tagged = enums.alloc(EnumDef {
        exact_type: crate::tests::test_physical_exact(
            "Tagged",
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: "Tagged".to_string(),
        repr: EnumRepr::Tagged {
            variants: vec![
                EnumVariantRepr {
                    fields: Vec::new(),
                    slot_offset: 8,
                    slot_size: 0,
                    slot_align: 1,
                    gc_free: true,
                },
                EnumVariantRepr {
                    fields: vec![EnumFieldRepr {
                        ty: LirType::Ptr(PointerKind::Managed),
                        offset: 8,
                    }],
                    slot_offset: 8,
                    slot_size: 8,
                    slot_align: 8,
                    gc_free: false,
                },
            ],
            size: 16,
            align: 8,
        },
        scan: RefScan::References(vec![8]),
    });
    let niche = enums.alloc(EnumDef {
        exact_type: crate::tests::test_physical_exact(
            "RawOption",
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: "RawOption".to_string(),
        repr: EnumRepr::Niche {
            kind: NullNicheKind::Raw,
            payload_variant: 0,
        },
        scan: RefScan::None,
    });

    let tagged_payload = enums.variant_ref(tagged, 1).expect("valid tagged variant");
    assert_eq!(tagged_payload.definition(), tagged);
    assert_eq!(tagged_payload.index(), 1);
    assert!(enums.contains_variant(tagged_payload));
    assert!(enums.variant_ref(tagged, 2).is_none());

    let tagged_field = enums
        .variant_field_ref(tagged_payload, 0)
        .expect("valid tagged payload field");
    assert_eq!(tagged_field.variant(), tagged_payload);
    assert_eq!(tagged_field.definition(), tagged);
    assert_eq!(tagged_field.index(), 0);
    assert_eq!(
        enums.variant_field_type(tagged_field),
        Some(LirType::Ptr(PointerKind::Managed))
    );
    assert!(enums.variant_field_ref(tagged_payload, 1).is_none());

    let niche_payload = enums.variant_ref(niche, 0).expect("valid niche payload");
    let niche_unit = enums.variant_ref(niche, 1).expect("valid niche unit");
    let niche_field = enums
        .variant_field_ref(niche_payload, 0)
        .expect("niche carrier is its single payload field");
    assert_eq!(
        enums.variant_field_type(niche_field),
        Some(LirType::Ptr(PointerKind::Raw))
    );
    assert!(enums.variant_field_ref(niche_payload, 1).is_none());
    assert!(enums.variant_field_ref(niche_unit, 0).is_none());
}

#[test]
fn non_empty_ref_scan_rejects_programs_without_references() {
    assert!(NonEmptyRefScan::new(RefScan::None).is_none());
    assert!(NonEmptyRefScan::new(RefScan::References(Vec::new())).is_none());
    assert!(
        NonEmptyRefScan::new(RefScan::Sequence(vec![
            RefScan::None,
            RefScan::References(Vec::new()),
        ]))
        .is_none()
    );
}

#[test]
fn non_empty_ref_scan_accepts_nested_references() {
    let scan = RefScan::Sequence(vec![
        RefScan::None,
        RefScan::Sequence(vec![RefScan::References(vec![16])]),
    ]);
    let scan = NonEmptyRefScan::new(scan).expect("nested reference makes the scan non-empty");

    assert_eq!(
        scan.as_ref_scan(),
        &RefScan::Sequence(vec![
            RefScan::None,
            RefScan::Sequence(vec![RefScan::References(vec![16])]),
        ])
    );
}

#[test]
fn local_function_registry_produces_effect_refined_identities() {
    let mut functions = LocalFunctionIdentities::default();
    let managed = functions.alloc_managed();
    let no_gc = functions.alloc_no_gc();

    assert_eq!(managed.declaration().into_u32(), 0);
    assert_eq!(no_gc.declaration().into_u32(), 1);
    assert!(matches!(
        ManagedCallDestination::local(managed).view(),
        CallDestination::Local(id) if id == managed.declaration()
    ));
    assert!(matches!(
        super::NoGcCallDestination::local(no_gc).view(),
        CallDestination::Local(id) if id == no_gc.declaration()
    ));
}

#[test]
fn external_destinations_are_effect_refined() {
    let callable = super::ExternalCallableId::from_raw(0_u32.into());
    assert!(matches!(
        ManagedCallDestination::external(callable).view(),
        CallDestination::External(id) if id == callable
    ));
    assert!(matches!(
        super::NoGcCallDestination::external(callable).view(),
        CallDestination::External(id) if id == callable
    ));
}

#[test]
fn typed_targets_atomically_bind_protocol_return_convention_and_signature() {
    let mut targets = CallTargets::default();
    let void_signature = targets
        .void_signatures
        .alloc(VoidCallSignature::new(Vec::new(), CallingConvention::Cdecl));
    let void_target = targets.managed_targets.void.alloc(CallTarget {
        destination: ManagedCallDestination::runtime(ManagedRuntimeFunction::GcCollect),
        signature: void_signature,
    });
    let void_call = TypedCall::Void {
        target: void_target,
        args: Vec::new(),
    };

    let i64_value = abi_value(LirType::I64, 8, 8, RefScan::None);
    let direct_signature = targets.direct_signatures.alloc(DirectCallSignature::new(
        vec![AbiArgument::Direct(i64_value.clone().into())],
        i64_value,
        CallingConvention::Cdecl,
    ));
    let mut local_functions = LocalFunctionIdentities::default();
    let local_function = (0..=7)
        .map(|_| local_functions.alloc_managed())
        .last()
        .expect("the test declares one local function");
    let direct_target = targets.managed_targets.direct.alloc(CallTarget {
        destination: ManagedCallDestination::local(local_function),
        signature: direct_signature,
    });
    let direct_call = TypedCall::Direct {
        target: direct_target,
        out: super::TempId::from_raw(la_arena::RawIdx::from_u32(0)),
        args: vec![AbiCallArgument::Direct(Value::IntegerConst(
            super::LirIntegerConstant::Signed64(1),
        ))],
    };

    let zst = abi_zst(LirType::Aggregate(Vec::new()), 1);
    let elided_signature = targets
        .elided_zst_signatures
        .alloc(ElidedZstCallSignature::new(
            Vec::new(),
            zst,
            CallingConvention::Cdecl,
        ));
    let elided_target = targets.managed_targets.elided_zst.alloc(CallTarget {
        destination: ManagedCallDestination::local(local_function),
        signature: elided_signature,
    });
    let elided_out = super::TempId::from_raw(la_arena::RawIdx::from_u32(1));
    let elided_call = TypedCall::ElidedZst {
        target: elided_target,
        out: elided_out,
        args: Vec::new(),
    };

    let void_view = targets.typed_call_view(
        &void_call,
        &targets.managed_targets,
        ManagedCallDestination::view,
    );
    let direct_view = targets.typed_call_view(
        &direct_call,
        &targets.managed_targets,
        ManagedCallDestination::view,
    );
    let elided_view = targets.typed_call_view(
        &elided_call,
        &targets.managed_targets,
        ManagedCallDestination::view,
    );

    let TypedCallView::Void {
        destination,
        signature,
        ..
    } = void_view
    else {
        panic!("void target must preserve its return arm");
    };
    assert!(matches!(destination, CallDestination::Runtime(_)));
    assert!(signature.arguments().is_empty());

    let TypedCallView::Direct {
        destination,
        signature,
        ..
    } = direct_view
    else {
        panic!("direct target must preserve its return arm");
    };
    assert!(matches!(destination, CallDestination::Local(_)));
    assert!(matches!(signature.arguments(), [AbiArgument::Direct(_)]));
    assert_eq!(signature.result().storage_type(), &LirType::I64);

    assert_eq!(elided_view.result(), TypedCallResult::ElidedZst(elided_out));
    let TypedCallView::ElidedZst { signature, .. } = elided_view else {
        panic!("non-Unit ZST must not collapse into the void arm");
    };
    assert_eq!(
        signature.result().storage_type(),
        &LirType::Aggregate(Vec::new())
    );
}

#[test]
fn native_borrowed_result_publication_is_sealed_with_return_convention() {
    let mut functions = ExternFunctions::default();
    let result_scan = RefScan::References(vec![0]);
    let direct_result = abi_value(super::MANAGED_PTR, 8, 8, result_scan.clone());
    let direct_function = functions.alloc_scoop(ScoopExternFunction {
        identity: ExternFunctionIdentity {
            source_name: "borrowed_direct".to_string(),
            native_symbol: "native_borrowed_direct".to_string(),
            library: "test".to_string(),
            calling_convention: CallingConvention::Cdecl,
        },
        gc_effect: GcEffect::Managed,
        signature: ScoopAbiSignature::new(
            Vec::new(),
            AbiReturn::Direct(direct_result.clone().into()),
            CallingConvention::Cdecl,
        ),
    });
    let direct_destination = NativeBorrowedCallDestination::extern_function(direct_function);
    let mut targets = CallTargets::default();

    let direct_signature = targets.direct_signatures.alloc(DirectCallSignature::new(
        Vec::new(),
        direct_result,
        CallingConvention::Cdecl,
    ));
    let direct_target = targets.native_borrowed_targets.direct.alloc(CallTarget {
        destination: direct_destination,
        signature: direct_signature,
    });
    let direct_storage = super::LocalId::from_raw(la_arena::RawIdx::from_u32(0));
    let direct = targets.bind_native_borrowed_call(
        TypedCall::Direct {
            target: direct_target,
            out: super::TempId::from_raw(la_arena::RawIdx::from_u32(0)),
            args: Vec::new(),
        },
        NativeBorrowedResultRoot::Rooted {
            storage: direct_storage,
        },
    );
    let direct = direct.view(&targets);
    assert!(matches!(direct.call, TypedCallView::Direct { .. }));
    assert!(matches!(
        direct.result,
        NativeBorrowedResultPublication::DirectRooted { storage, scan }
            if storage == direct_storage && scan.as_ref_scan() == &result_scan
    ));

    let zst_result = abi_zst(LirType::Aggregate(Vec::new()), 8);
    let elided_function = functions.alloc_scoop(ScoopExternFunction {
        identity: ExternFunctionIdentity {
            source_name: "borrowed_zst".to_string(),
            native_symbol: "native_borrowed_zst".to_string(),
            library: "test".to_string(),
            calling_convention: CallingConvention::Cdecl,
        },
        gc_effect: GcEffect::Managed,
        signature: ScoopAbiSignature::new(
            Vec::new(),
            AbiReturn::ElidedZst(zst_result.clone()),
            CallingConvention::Cdecl,
        ),
    });
    let elided_signature = targets
        .elided_zst_signatures
        .alloc(ElidedZstCallSignature::new(
            Vec::new(),
            zst_result,
            CallingConvention::Cdecl,
        ));
    let elided_target = targets
        .native_borrowed_targets
        .elided_zst
        .alloc(CallTarget {
            destination: NativeBorrowedCallDestination::extern_function(elided_function),
            signature: elided_signature,
        });
    let elided_out = super::TempId::from_raw(la_arena::RawIdx::from_u32(1));
    let elided = targets.bind_native_borrowed_call(
        TypedCall::ElidedZst {
            target: elided_target,
            out: elided_out,
            args: Vec::new(),
        },
        NativeBorrowedResultRoot::GcFree,
    );
    let elided = elided.view(&targets);
    assert_eq!(elided.call.result(), TypedCallResult::ElidedZst(elided_out));
    assert!(matches!(
        elided.result,
        NativeBorrowedResultPublication::ElidedZst
    ));

    let indirect_storage = super::LocalId::from_raw(la_arena::RawIdx::from_u32(1));
    let indirect_result = abi_value(super::MANAGED_PTR, 8, 8, result_scan.clone());
    let indirect_function = functions.alloc_scoop(ScoopExternFunction {
        identity: ExternFunctionIdentity {
            source_name: "borrowed_indirect".to_string(),
            native_symbol: "native_borrowed_indirect".to_string(),
            library: "test".to_string(),
            calling_convention: CallingConvention::Cdecl,
        },
        gc_effect: GcEffect::Managed,
        signature: ScoopAbiSignature::new(
            Vec::new(),
            AbiReturn::Indirect(indirect_result.clone()),
            CallingConvention::Cdecl,
        ),
    });
    let indirect_signature =
        targets
            .indirect_result_signatures
            .alloc(IndirectResultCallSignature::sret(
                Vec::new(),
                indirect_result,
                CallingConvention::Cdecl,
            ));
    let indirect_target = targets
        .native_borrowed_targets
        .indirect_result
        .alloc(CallTarget {
            destination: NativeBorrowedCallDestination::extern_function(indirect_function),
            signature: indirect_signature,
        });
    let indirect = targets.bind_native_borrowed_call(
        TypedCall::IndirectResult {
            target: indirect_target,
            storage: indirect_storage,
            args: Vec::new(),
        },
        NativeBorrowedResultRoot::Rooted {
            storage: indirect_storage,
        },
    );
    let indirect = indirect.view(&targets);
    let TypedCallView::IndirectResult { signature, .. } = indirect.call else {
        panic!("indirect target must preserve its return arm");
    };
    assert_eq!(signature.convention(), IndirectResultConvention::Sret);
    assert!(matches!(
        indirect.result,
        NativeBorrowedResultPublication::IndirectResultRooted { storage, scan }
            if storage == indirect_storage && scan.as_ref_scan() == &result_scan
    ));
}

#[test]
fn indirect_result_signatures_keep_scoop_and_c_pointer_conventions_distinct() {
    let result = abi_value(
        LirType::Aggregate(vec![LirType::I64, LirType::I64]),
        16,
        8,
        RefScan::None,
    );
    let scoop =
        IndirectResultCallSignature::sret(Vec::new(), result.clone(), CallingConvention::Cdecl);
    let c_bridge = IndirectResultCallSignature::c_storage_pointer(
        Vec::new(),
        result,
        CallingConvention::Cdecl,
    );

    assert_eq!(scoop.convention(), IndirectResultConvention::Sret);
    assert_eq!(
        c_bridge.convention(),
        IndirectResultConvention::CStoragePointer
    );
    assert_eq!(scoop.result(), c_bridge.result());
}

#[test]
fn function_parameters_keep_logical_types_across_abi_conventions() {
    let zst_ty = LirType::Aggregate(Vec::new());
    let indirect_ty = LirType::Aggregate(vec![LirType::I64, LirType::I64]);
    let signature = ScoopAbiSignature::new(
        vec![
            AbiArgument::ElidedZst(abi_zst(zst_ty.clone(), 1)),
            AbiArgument::Direct(abi_value(LirType::I64, 8, 8, RefScan::None).into()),
            AbiArgument::Indirect(abi_value(indirect_ty.clone(), 16, 8, RefScan::None)),
        ],
        AbiReturn::UnitVoid,
        CallingConvention::Cdecl,
    );
    let mut blocks = la_arena::Arena::new();
    let entry = blocks.alloc(super::BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: super::Terminator::Return { value: None },
    });
    let function = super::Function {
        callable_body: callable_body("logical_params"),
        gc_effect: GcEffect::NoGc,
        signature,
        call_targets: CallTargets::default(),
        safepoints: super::SafepointIdentities::default(),
        locals: la_arena::Arena::new(),
        temps: la_arena::Arena::new(),
        blocks,
        entry,
    };
    let globals = la_arena::Arena::new();

    assert_eq!(function.value_ty(&globals, Value::Param(0)), zst_ty);
    assert_eq!(function.value_ty(&globals, Value::Param(1)), LirType::I64);
    assert_eq!(function.value_ty(&globals, Value::Param(2)), indirect_ty);
}

#[test]
fn extern_references_are_refined_by_abi_before_entering_call_targets() {
    let mut functions = ExternFunctions::default();
    let native_symbol = scoop_identity::NativeExternalSymbolKey::darwin_macho_external(
        &scoop_identity::SourceNativeSymbol::new("c").unwrap(),
    )
    .unwrap();
    let external_contract = scoop_identity::NativeExternalContract::c_function(
        scoop_identity::NativeLibraryBinding::DefaultNativeNamespace,
        scoop_identity::CanonicalCAbiFunctionSignature::cdecl(
            Vec::new(),
            scoop_identity::CanonicalCAbiReturn::Void,
        ),
    );
    let contract = scoop_identity::NativeExternalContractFingerprint::from_symbol_and_contract(
        scoop_identity::PersistentNativeExternalSymbolId::from_key(&native_symbol).unwrap(),
        &external_contract,
    )
    .unwrap();
    let c_ref: CExternFunctionRef = functions.alloc_c(CExternFunction {
        call_mode: scoop_identity::CAbiCallMode::NativeSafe,
        identity: ExternFunctionIdentity {
            source_name: "c".to_string(),
            native_symbol: "c".to_string(),
            library: "test".to_string(),
            calling_convention: CallingConvention::Cdecl,
        },
        call_plan: super::CAbiCallPlan::StorageBridge {
            entry: Box::new(
                super::GeneratedBridgeEntryIdentity::new(
                    scoop_identity::ConeIdentity::SINGLE_FILE,
                    scoop_identity::GeneratedBridgeUnitKey::OutboundFunction(
                        contract,
                        scoop_identity::CResultAdaptation::Direct,
                    ),
                )
                .unwrap(),
            ),
            result: scoop_identity::CResultAdaptation::Direct,
        },
        signature: super::CFunctionType {
            params: Vec::new(),
            return_type: super::CReturnType::Void,
        },
    });
    let scoop_ref: ScoopExternFunctionRef = functions.alloc_scoop(ScoopExternFunction {
        identity: ExternFunctionIdentity {
            source_name: "scoop".to_string(),
            native_symbol: "scoop".to_string(),
            library: "test".to_string(),
            calling_convention: CallingConvention::Cdecl,
        },
        gc_effect: GcEffect::Managed,
        signature: ScoopAbiSignature::new(
            Vec::new(),
            AbiReturn::UnitVoid,
            CallingConvention::Cdecl,
        ),
    });

    let c = c_ref.declaration();
    let scoop = scoop_ref.declaration();
    assert!(matches!(
        functions[c].kind,
        super::ExternFunctionKind::C { .. }
    ));
    assert!(matches!(
        functions[scoop].kind,
        super::ExternFunctionKind::Scoop { .. }
    ));
    assert_eq!(
        CCallDestination::extern_function(c_ref).view(),
        CallDestination::Extern(c)
    );
    assert_eq!(
        NativeBorrowedCallDestination::extern_function(scoop_ref).view(),
        CallDestination::Extern(scoop)
    );
}

#[test]
fn machine_scalars_keep_closed_domains_and_frozen_i64_encodings() {
    assert!(CoroutineSuspendStateId::new(0).is_none());
    let state = CoroutineSuspendStateId::new(3).expect("three is nonzero");
    let first = CoroutineSuspendStateId::new(1).expect("one is nonzero");
    let last = CoroutineSuspendStateId::new(u32::MAX).expect("u32::MAX is nonzero");

    let frame_encodings = [
        MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Initial).raw_bits(),
        MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Running).raw_bits(),
        MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Completed).raw_bits(),
        MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Suspended(first)).raw_bits(),
        MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Suspended(last)).raw_bits(),
        MachineScalarValue::CoroutineFrameState(CoroutineFrameState::ResumeFailure(first))
            .raw_bits(),
        MachineScalarValue::CoroutineFrameState(CoroutineFrameState::ResumeFailure(last))
            .raw_bits(),
    ];
    for (index, encoding) in frame_encodings.iter().enumerate() {
        assert!(
            frame_encodings[index + 1..]
                .iter()
                .all(|other| other != encoding),
            "coroutine frame state encodings must be pairwise distinct"
        );
    }

    let cases = [
        (
            MachineScalarValue::ByteSize(24),
            MachineScalarKind::ByteSize,
            24,
        ),
        (
            MachineScalarValue::EnumTag(2),
            MachineScalarKind::EnumTag,
            2,
        ),
        (
            MachineScalarValue::InitializationOutcome(InitializationOutcome::Cycle),
            MachineScalarKind::InitializationOutcome,
            3,
        ),
        (
            MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Suspended(state)),
            MachineScalarKind::CoroutineFrameState,
            3,
        ),
        (
            MachineScalarValue::CoroutineFrameState(CoroutineFrameState::ResumeFailure(state)),
            MachineScalarKind::CoroutineFrameState,
            u64::MAX - 4,
        ),
        (
            MachineScalarValue::CoroutineAdapterState(CoroutineAdapterState::Consumed),
            MachineScalarKind::CoroutineAdapterState,
            6,
        ),
        (
            MachineScalarValue::ForeignCallbackStatus(ForeignCallbackStatus::Threw),
            MachineScalarKind::ForeignCallbackStatus,
            1,
        ),
        (
            MachineScalarValue::PointerElementOffset(7),
            MachineScalarKind::PointerElementOffset,
            7,
        ),
    ];

    for (value, kind, bits) in cases {
        assert_eq!(value.kind(), kind);
        assert_eq!(value.raw_bits(), bits);
    }
    assert_ne!(
        LirType::MachineScalar(MachineScalarKind::EnumTag).dump(),
        LirType::I64.dump()
    );
}

#[test]
fn source_integer_kinds_have_exact_width_identity() {
    use super::{IntegerKind, IntegerSignedness, IntegerWidth};

    let expected = [
        (
            IntegerKind::SIGNED_8,
            IntegerSignedness::Signed,
            IntegerWidth::W8,
            "Int8",
            LirType::I8,
        ),
        (
            IntegerKind::SIGNED_16,
            IntegerSignedness::Signed,
            IntegerWidth::W16,
            "Int16",
            LirType::I16,
        ),
        (
            IntegerKind::SIGNED_32,
            IntegerSignedness::Signed,
            IntegerWidth::W32,
            "Int",
            LirType::I32,
        ),
        (
            IntegerKind::SIGNED_64,
            IntegerSignedness::Signed,
            IntegerWidth::W64,
            "Long",
            LirType::I64,
        ),
        (
            IntegerKind::UNSIGNED_8,
            IntegerSignedness::Unsigned,
            IntegerWidth::W8,
            "UInt8",
            LirType::I8,
        ),
        (
            IntegerKind::UNSIGNED_16,
            IntegerSignedness::Unsigned,
            IntegerWidth::W16,
            "UInt16",
            LirType::I16,
        ),
        (
            IntegerKind::UNSIGNED_32,
            IntegerSignedness::Unsigned,
            IntegerWidth::W32,
            "UInt",
            LirType::I32,
        ),
        (
            IntegerKind::UNSIGNED_64,
            IntegerSignedness::Unsigned,
            IntegerWidth::W64,
            "ULong",
            LirType::I64,
        ),
    ];

    assert_eq!(IntegerKind::ALL.len(), expected.len());
    for (kind, signedness, width, name, scalar) in expected {
        assert_eq!(kind.signedness(), signedness);
        assert_eq!(kind.width(), width);
        assert_eq!(kind.canonical_name(), name);
        assert_eq!(kind.scalar_type(), scalar);
        assert_eq!(width.bytes(), u64::from(width.bits() / 8));
        assert_eq!(width.shift_mask(), u64::from(width.bits() - 1));
    }
}

#[test]
fn source_integer_constant_variants_are_their_own_width_witnesses() {
    use super::{IntegerKind, LirIntegerConstant};

    let constants = [
        (
            LirIntegerConstant::Signed8(u8::MAX),
            IntegerKind::SIGNED_8,
            LirType::I8,
            0xff,
        ),
        (
            LirIntegerConstant::Signed16(u16::MAX),
            IntegerKind::SIGNED_16,
            LirType::I16,
            0xffff,
        ),
        (
            LirIntegerConstant::Signed32(u32::MAX),
            IntegerKind::SIGNED_32,
            LirType::I32,
            0xffff_ffff,
        ),
        (
            LirIntegerConstant::Signed64(u64::MAX),
            IntegerKind::SIGNED_64,
            LirType::I64,
            u64::MAX,
        ),
        (
            LirIntegerConstant::Unsigned8(u8::MAX),
            IntegerKind::UNSIGNED_8,
            LirType::I8,
            0xff,
        ),
        (
            LirIntegerConstant::Unsigned16(u16::MAX),
            IntegerKind::UNSIGNED_16,
            LirType::I16,
            0xffff,
        ),
        (
            LirIntegerConstant::Unsigned32(u32::MAX),
            IntegerKind::UNSIGNED_32,
            LirType::I32,
            0xffff_ffff,
        ),
        (
            LirIntegerConstant::Unsigned64(u64::MAX),
            IntegerKind::UNSIGNED_64,
            LirType::I64,
            u64::MAX,
        ),
    ];

    for (constant, kind, scalar, bits) in constants {
        assert_eq!(constant.kind(), kind);
        assert_eq!(constant.scalar_type(), scalar);
        assert_eq!(constant.raw_bits(), bits);
        assert!(constant.dump().contains(kind.canonical_name()));
    }
}

#[test]
fn c_integer_classifier_and_c_layout_contract_are_closed() {
    use super::{CType, IntegerKind, LirCLayoutContract, LirCLayoutValue};

    assert_ne!(
        CType::Integer(IntegerKind::SIGNED_32),
        CType::Integer(IntegerKind::UNSIGNED_32)
    );
    let contract = LirCLayoutContract {
        aligned: LirCLayoutValue::A16,
        packed: LirCLayoutValue::A2,
    };
    assert_eq!(contract.aligned.bytes(), Some(16));
    assert_eq!(contract.packed.bytes(), Some(2));
    assert_eq!(LirCLayoutValue::Natural.bytes(), None);
}

#[test]
fn exact_c_types_totally_determine_their_lir_storage() {
    use super::{
        CCodePointerStorage, CDataPointee, CDataPointerStorage, CFunctionType, CReturnType, CType,
        EnumDef, EnumDefs, IntegerKind, LirCLayoutContract, LirCLayoutValue, StructDefs,
    };

    let mut enums = EnumDefs::default();
    let raw_nullable = enums.alloc_c_nullable_data_pointer_option(EnumDef {
        exact_type: crate::tests::test_physical_exact(
            "Option<Ptr<Unit>>",
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: "Option<Ptr<Unit>>".to_string(),
        repr: EnumRepr::Niche {
            kind: NullNicheKind::Raw,
            payload_variant: 0,
        },
        scan: RefScan::None,
    });
    let raw_nullable_id = raw_nullable;
    let code_nullable = enums.alloc_c_nullable_code_pointer_option(EnumDef {
        exact_type: crate::tests::test_physical_exact(
            "Option<FunPtr<() -> Unit>>",
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: "Option<FunPtr<() -> Unit>>".to_string(),
        repr: EnumRepr::Niche {
            kind: NullNicheKind::Code,
            payload_variant: 0,
        },
        scan: RefScan::None,
    });
    let code_nullable_id = code_nullable;
    let mut structs = StructDefs::default();
    let c_struct = structs.alloc_c(
        crate::tests::test_physical_exact("CValue", scoop_identity::SourceNominalKind::Struct),
        "CValue".to_string(),
        4,
        4,
        false,
        LirCLayoutContract {
            aligned: LirCLayoutValue::Natural,
            packed: LirCLayoutValue::Natural,
        },
        Vec::new(),
    );
    let c_struct_id = c_struct.definition();
    let signature = CFunctionType {
        params: vec![CType::Integer(IntegerKind::SIGNED_8)],
        return_type: CReturnType::Void,
    };
    let raw_nullable_ref = enums
        .nullable_data_pointer_ref(raw_nullable_id, CDataPointee::OpaqueVoid)
        .expect("raw niche ref");
    let code_nullable_ref = enums
        .nullable_code_pointer_ref(code_nullable_id, signature.clone())
        .expect("code niche ref");
    assert_eq!(raw_nullable_ref.pointee(), &CDataPointee::OpaqueVoid);
    assert_eq!(code_nullable_ref.signature(), &signature);
    assert!(
        enums
            .nullable_data_pointer_ref(
                raw_nullable_id,
                CDataPointee::Object(Box::new(CType::Integer(IntegerKind::SIGNED_32))),
            )
            .is_none(),
        "an exact Option<Ptr<Unit>> binding cannot be rebound as Option<Ptr<Int>>"
    );
    assert!(
        enums
            .nullable_code_pointer_ref(
                code_nullable_id,
                CFunctionType {
                    params: Vec::new(),
                    return_type: CReturnType::Value(Box::new(CType::Integer(
                        IntegerKind::SIGNED_32,
                    ))),
                },
            )
            .is_none(),
        "an exact nullable code-pointer binding cannot be rebound to another signature"
    );
    assert_eq!(
        CType::DataPointer {
            pointee: CDataPointee::OpaqueVoid,
            storage: CDataPointerStorage::Nullable(raw_nullable_ref.clone()),
        }
        .dump(),
        "data-ptr<opaque-void,nullable=enum0<opaque-void>>"
    );
    assert_eq!(
        CType::CodePointer {
            signature: Box::new(signature.clone()),
            storage: CCodePointerStorage::Nullable(code_nullable_ref.clone()),
        }
        .dump(),
        "code-ptr<(Int8)->void,nullable=enum1<(Int8)->void>>"
    );
    for (c_type, storage) in [
        (CType::Integer(IntegerKind::UNSIGNED_16), LirType::I16),
        (CType::Boolean, LirType::I1),
        (
            CType::DataPointer {
                pointee: CDataPointee::OpaqueVoid,
                storage: CDataPointerStorage::Direct,
            },
            LirType::Ptr(PointerKind::Raw),
        ),
        (
            CType::DataPointer {
                pointee: CDataPointee::OpaqueVoid,
                storage: CDataPointerStorage::Nullable(raw_nullable_ref),
            },
            LirType::Enum(raw_nullable_id),
        ),
        (
            CType::CodePointer {
                signature: Box::new(signature.clone()),
                storage: CCodePointerStorage::Direct,
            },
            LirType::Ptr(PointerKind::Code),
        ),
        (
            CType::CodePointer {
                signature: Box::new(signature),
                storage: CCodePointerStorage::Nullable(code_nullable_ref),
            },
            LirType::Enum(code_nullable_id),
        ),
        (CType::Struct(c_struct), LirType::Struct(c_struct_id)),
    ] {
        assert_eq!(c_type.storage_type(), storage);
    }
    assert_eq!(CReturnType::Void.storage_type(), LirType::Void);
}

#[test]
fn target_contract_and_fingerprint_match_the_fixed_vectors() {
    let profile = LirTargetProfile::DARWIN_AARCH64;
    assert_eq!(
        hex(&scoop_wire::encode(&profile.contract()).unwrap()),
        "af0174616172636836342d6170706c652d64617277696e027847652d6d3a6f2d703237303a33323a33322d703237313a33323a33322d703237323a36343a36342d6936343a36342d693132383a3132382d6e33323a36342d533132382d466e333203a301781c6f72672e73636f6f702d6c616e672e6f626a6563742d666f726d617402726d6163682d6f2d72656c6f63617461626c65030104010585a3010102010301a3010202010301a3010302020302a3010402040304a301050208030806a20108020807a4010802080301040108a4010802080301040109a2010802080a100b100c1b7fffffffffffffff0d030e030f01"
    );
    assert_eq!(
        profile.fingerprint().unwrap().to_string(),
        "e028c9fb172ca19a32f8b2e157a049608c6ceac14711e267b1a3a1647d1705ce"
    );
    assert_eq!(
        super::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1.target(),
        profile
    );
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
