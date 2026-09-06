use super::{
    CExternFunction, CExternFunctionRef, CallDestination, CallTarget, CallTargets,
    CallingConvention, CoroutineAdapterState, CoroutineFrameState, CoroutineSuspendStateId,
    DirectCallSignature, EnumDef, EnumDefs, EnumFieldRepr, EnumRepr, EnumVariantRepr,
    ExternFunctionIdentity, ExternFunctions, ForeignCallbackStatus, GcEffect,
    InitializationOutcome, InternalPointerCarrier, LirFunctionType, LirReturnType,
    LirTargetProfile, LirType, LocalFunctionIdentities, MachineScalarKind, MachineScalarValue,
    ManagedCallDestination, ManagedRuntimeFunction, NativeBorrowedCallDestination,
    NativeBorrowedResultPublication, NativeBorrowedResultRoot, NativeSafeCallDestination,
    NichePointerKind, NonEmptyRefScan, PointerKind, PointerNullEncoding, RefScan, ResultStorage,
    ScoopExternFunction, ScoopExternFunctionRef, TargetProfileId, TypedCall, TypedCallView, Value,
    VoidCallSignature,
};

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
        (NichePointerKind::Managed, PointerKind::Managed),
        (NichePointerKind::Raw, PointerKind::Raw),
        (NichePointerKind::Code, PointerKind::Code),
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

        assert_eq!(kind.pointer_kind(), expected);
        assert_eq!(PointerKind::from(kind), expected);
        assert_eq!(payload_variant, 1);
    }
}

#[test]
fn enum_store_is_the_only_checked_variant_and_payload_field_ref_producer() {
    let mut enums = EnumDefs::default();
    let tagged = enums.alloc(EnumDef {
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
        name: "RawOption".to_string(),
        repr: EnumRepr::Niche {
            kind: NichePointerKind::Raw,
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
fn typed_targets_atomically_bind_protocol_return_convention_and_signature() {
    let mut targets = CallTargets::default();
    let void_signature = targets.void_signatures.alloc(VoidCallSignature {
        params: Vec::new(),
        calling_convention: CallingConvention::Cdecl,
    });
    let void_target = targets.managed_targets.void.alloc(CallTarget {
        destination: ManagedCallDestination::runtime(ManagedRuntimeFunction::GcCollect),
        signature: void_signature,
    });
    let void_call = TypedCall::Void {
        target: void_target,
        args: Vec::new(),
    };

    let direct_signature = targets.direct_signatures.alloc(DirectCallSignature {
        params: vec![LirType::I64],
        result: LirType::I64,
        result_scan: RefScan::None,
        calling_convention: CallingConvention::Cdecl,
    });
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
        args: vec![Value::IntegerConst(super::LirIntegerConstant::Signed64(1))],
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

    assert!(matches!(
        void_view,
        TypedCallView::Void {
            destination: CallDestination::Runtime(_),
            signature: VoidCallSignature { params, .. },
            ..
        } if params.is_empty()
    ));
    assert!(matches!(
        direct_view,
        TypedCallView::Direct {
            destination: CallDestination::Local(_),
            signature: DirectCallSignature { params, result: LirType::I64, .. },
            ..
        } if params == &[LirType::I64]
    ));
}

#[test]
fn native_borrowed_result_publication_is_sealed_with_return_convention() {
    let mut functions = ExternFunctions::default();
    let function = functions.alloc_scoop(ScoopExternFunction {
        identity: ExternFunctionIdentity {
            source_name: "borrowed".to_string(),
            native_symbol: "native_borrowed".to_string(),
            library: "test".to_string(),
            calling_convention: CallingConvention::Cdecl,
        },
        gc_effect: GcEffect::Managed,
        signature: LirFunctionType {
            params: Vec::new(),
            return_type: LirReturnType::Value(Box::new(super::MANAGED_PTR)),
        },
    });
    let destination = NativeBorrowedCallDestination::extern_function(function);
    let mut targets = CallTargets::default();
    let result_scan = RefScan::References(vec![0]);

    let direct_signature = targets.direct_signatures.alloc(DirectCallSignature {
        params: Vec::new(),
        result: super::MANAGED_PTR,
        result_scan: result_scan.clone(),
        calling_convention: CallingConvention::Cdecl,
    });
    let direct_target = targets.native_borrowed_targets.direct.alloc(CallTarget {
        destination,
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

    let indirect_storage = super::LocalId::from_raw(la_arena::RawIdx::from_u32(1));
    let indirect_signature =
        targets
            .indirect_result_signatures
            .alloc(super::IndirectResultCallSignature {
                params: Vec::new(),
                result: ResultStorage {
                    ty: super::MANAGED_PTR,
                    scan: result_scan.clone(),
                },
                calling_convention: CallingConvention::Cdecl,
            });
    let indirect_target = targets
        .native_borrowed_targets
        .indirect_result
        .alloc(CallTarget {
            destination,
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
    assert!(matches!(
        indirect.result,
        NativeBorrowedResultPublication::IndirectResultRooted { storage, scan }
            if storage == indirect_storage && scan.as_ref_scan() == &result_scan
    ));
}

#[test]
fn extern_references_are_refined_by_abi_before_entering_call_targets() {
    let mut functions = ExternFunctions::default();
    let c_ref: CExternFunctionRef = functions.alloc_c(CExternFunction {
        identity: ExternFunctionIdentity {
            source_name: "c".to_string(),
            native_symbol: "c".to_string(),
            library: "test".to_string(),
            calling_convention: CallingConvention::Cdecl,
        },
        bridge_symbol: "c_bridge".to_string(),
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
        signature: LirFunctionType {
            params: Vec::new(),
            return_type: LirReturnType::Void,
        },
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
        NativeSafeCallDestination::extern_function(c_ref).view(),
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
fn source_integer_kinds_have_exact_width_identity_and_compact_v2_codes() {
    use super::{IntegerKind, IntegerSignedness, IntegerWidth};

    let expected = [
        (
            IntegerKind::SIGNED_8,
            IntegerSignedness::Signed,
            IntegerWidth::W8,
            "Int8",
            "I8",
            LirType::I8,
        ),
        (
            IntegerKind::SIGNED_16,
            IntegerSignedness::Signed,
            IntegerWidth::W16,
            "Int16",
            "I16",
            LirType::I16,
        ),
        (
            IntegerKind::SIGNED_32,
            IntegerSignedness::Signed,
            IntegerWidth::W32,
            "Int",
            "I32",
            LirType::I32,
        ),
        (
            IntegerKind::SIGNED_64,
            IntegerSignedness::Signed,
            IntegerWidth::W64,
            "Long",
            "I64",
            LirType::I64,
        ),
        (
            IntegerKind::UNSIGNED_8,
            IntegerSignedness::Unsigned,
            IntegerWidth::W8,
            "UInt8",
            "V8",
            LirType::I8,
        ),
        (
            IntegerKind::UNSIGNED_16,
            IntegerSignedness::Unsigned,
            IntegerWidth::W16,
            "UInt16",
            "V16",
            LirType::I16,
        ),
        (
            IntegerKind::UNSIGNED_32,
            IntegerSignedness::Unsigned,
            IntegerWidth::W32,
            "UInt",
            "V32",
            LirType::I32,
        ),
        (
            IntegerKind::UNSIGNED_64,
            IntegerSignedness::Unsigned,
            IntegerWidth::W64,
            "ULong",
            "V64",
            LirType::I64,
        ),
    ];

    assert_eq!(IntegerKind::ALL.len(), expected.len());
    for (kind, signedness, width, name, code, scalar) in expected {
        assert_eq!(kind.signedness(), signedness);
        assert_eq!(kind.width(), width);
        assert_eq!(kind.canonical_name(), name);
        assert_eq!(kind.compact_v2_code(), code);
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
        name: "Option<Ptr<Unit>>".to_string(),
        repr: EnumRepr::Niche {
            kind: NichePointerKind::Raw,
            payload_variant: 0,
        },
        scan: RefScan::None,
    });
    let raw_nullable_id = raw_nullable;
    let code_nullable = enums.alloc_c_nullable_code_pointer_option(EnumDef {
        name: "Option<FunPtr<() -> Unit>>".to_string(),
        repr: EnumRepr::Niche {
            kind: NichePointerKind::Code,
            payload_variant: 0,
        },
        scan: RefScan::None,
    });
    let code_nullable_id = code_nullable;
    let mut structs = StructDefs::default();
    let c_struct = structs.alloc_c(
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
