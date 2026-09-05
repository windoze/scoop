use super::{
    CExternFunction, CExternFunctionRef, CallDestination, CallTarget, CallTargets,
    CallingConvention, CoroutineAdapterState, CoroutineFrameState, CoroutineSuspendStateId,
    DirectCallSignature, EnumRepr, ExternFunctionDeclaration, ExternFunctions,
    ForeignCallbackStatus, GcEffect, InitializationOutcome, InternalPointerCarrier,
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
        args: vec![Value::IntConst(1)],
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
        declaration: ExternFunctionDeclaration {
            source_name: "borrowed".to_string(),
            native_symbol: "native_borrowed".to_string(),
            library: "test".to_string(),
            calling_convention: CallingConvention::Cdecl,
            params: Vec::new(),
            return_type: super::MANAGED_PTR,
        },
        gc_effect: GcEffect::Managed,
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
        declaration: ExternFunctionDeclaration {
            source_name: "c".to_string(),
            native_symbol: "c".to_string(),
            library: "test".to_string(),
            calling_convention: CallingConvention::Cdecl,
            params: Vec::new(),
            return_type: LirType::Void,
        },
        bridge_symbol: "c_bridge".to_string(),
        params: Vec::new(),
        return_type: super::CType::Unit,
    });
    let scoop_ref: ScoopExternFunctionRef = functions.alloc_scoop(ScoopExternFunction {
        declaration: ExternFunctionDeclaration {
            source_name: "scoop".to_string(),
            native_symbol: "scoop".to_string(),
            library: "test".to_string(),
            calling_convention: CallingConvention::Cdecl,
            params: Vec::new(),
            return_type: LirType::Void,
        },
        gc_effect: GcEffect::Managed,
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
