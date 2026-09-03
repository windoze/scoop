use super::{
    CExternFunction, CExternFunctionRef, CallDestination, CallTarget, CallTargets,
    CallingConvention, DirectCallSignature, ExternFunctionDeclaration, ExternFunctions, GcEffect,
    LirType, LocalFunctionIdentities, ManagedCallDestination, ManagedRuntimeFunction,
    NativeBorrowedCallDestination, NativeBorrowedResultPublication, NativeBorrowedResultRoot,
    NativeSafeCallDestination, NonEmptyRefScan, RefScan, ResultStorage, ScoopExternFunction,
    ScoopExternFunctionRef, TypedCall, TypedCallView, Value, VoidCallSignature,
};

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
