use super::*;

// ---- M9: GC support ----

#[test]
fn llvm_lowering_consumes_the_profile_managed_address_space() {
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let profile = host_profile().with_managed_address_space_for_test(7);
    let llvm = emit_llvm_module(&context, &values_module(), &machine, profile)
        .expect("emit module with supplied managed address space");
    llvm.verify().expect("valid LLVM module");
    let ir = llvm.print_to_string().to_string();
    assert!(
        ir.contains("ptr addrspace(7)"),
        "supplied managed address space is absent:\n{ir}"
    );
    assert!(
        !ir.contains("ptr addrspace(1)"),
        "lowering retained a hard-coded managed address space:\n{ir}"
    );
}

#[test]
fn typed_intrinsic_string_supplies_the_only_descriptor_definition() {
    let module = values_module();
    let string_symbol = type_descriptor_symbol(&module, "String");
    let definition = format!("@\"{string_symbol}\" =");
    let ir = ir_of(&module);
    assert_eq!(
        ir.match_indices(&definition).count(),
        1,
        "String must have exactly one descriptor definition"
    );
    assert!(ir.contains(&definition));
}

#[test]
fn low_level_codegen_does_not_emit_legacy_image_tables() {
    let mut module = values_module();
    let storage_identity = static_storage_identity("managedGlobal");
    module.globals.alloc(Global {
        address_kind: PointerKind::Raw,
        scan: RefScan::References(vec![0]),
        init: GlobalInit::Storage {
            identity: storage_identity,
            layout: layout_identity(
                "managedGlobal",
                scoop_identity::RepresentationRole::ManagedValue,
            )
            .into(),
            ty: MANAGED_PTR,
            initial_state: LirStaticInitialState::EncodedStaticValue {
                payload: LirConstantImage::NullPointer(PointerKind::Managed),
            },
        },
    });

    let ir = ir_of(&module);
    assert!(!ir.contains("@scoop_image_managed_globals"), "{ir}");
    assert!(!ir.contains("@scoop_image_managed_global_count"), "{ir}");
    assert!(!ir.contains("@scoop_image_immortal_objects"), "{ir}");
    assert!(!ir.contains("@scoop_image_immortal_object_count"), "{ir}");
}

#[test]
fn empty_module_does_not_emit_legacy_image_sentinels() {
    let module = enum_module();
    let ir = ir_of(&module);
    assert!(!ir.contains("@scoop_image_managed_globals"), "{ir}");
    assert!(!ir.contains("@scoop_image_immortal_objects"), "{ir}");
}

#[test]
fn raw_thread_local_global_rejects_managed_references() {
    let mut module = values_module();
    let identity = static_storage_identity("managedTls");
    module.globals.alloc(Global {
        address_kind: PointerKind::Raw,
        scan: RefScan::References(vec![0]),
        init: GlobalInit::RawStorage {
            identity,
            ty: MANAGED_PTR,
            initializer: LirConstantImage::NullPointer(PointerKind::Managed),
            thread_local: true,
        },
    });
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("raw TLS cannot contain managed references");
    assert!(
        error.0.contains("raw storage must be GC-free"),
        "unexpected error: {error}"
    );
}

#[test]
fn typed_local_call_signature_cannot_be_replaced_by_a_callsite_guess() {
    let mut module = exceptions_module();
    module.functions[1].signature = plain_scoop_signature(vec![LirType::I64], LirType::I64);
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("the local declaration and typed call target disagree");
    assert!(
        error
            .0
            .contains("signature or physical convention does not match"),
        "unexpected error: {error}"
    );
}

#[test]
fn typed_no_gc_effect_keeps_the_call_outside_statepoints() {
    let module = heap_module();
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let llvm = emit_llvm_module(&context, &module, &machine, host_profile()).expect("emit module");
    llvm.verify().expect("valid LLVM module");
    statepoint::rewrite(&llvm, &machine).expect("rewrite-statepoints-for-gc pass");
    let rewritten = llvm.print_to_string().to_string();
    assert!(
        rewritten
            .lines()
            .any(|line| line.contains("call i1 @scoop_rt_is_instance")),
        "NoGC call must remain an ordinary call after statepoint rewriting:\n{rewritten}"
    );
}

#[test]
fn native_calls_publish_roots_transition_and_reload() {
    let mut extern_functions = scoop_lir::ExternFunctions::default();
    let c_call = extern_functions.alloc_c(scoop_lir::CExternFunction {
        identity: scoop_lir::ExternFunctionIdentity {
            source_name: "wait".to_string(),
            native_symbol: "native_wait".to_string(),
            library: "fixture".to_string(),
            calling_convention: scoop_lir::CallingConvention::Cdecl,
        },
        bridge: outbound_bridge(1),
        signature: scoop_lir::CFunctionType {
            params: Vec::new(),
            return_type: scoop_lir::CReturnType::Void,
        },
    });
    let borrowed = extern_functions.alloc_scoop(scoop_lir::ScoopExternFunction {
        identity: scoop_lir::ExternFunctionIdentity {
            source_name: "borrowed".to_string(),
            native_symbol: "native_borrowed".to_string(),
            library: "fixture".to_string(),
            calling_convention: scoop_lir::CallingConvention::Cdecl,
        },
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(vec![MANAGED_PTR], MANAGED_PTR),
    });

    let mut safe_targets = CallTargets::default();
    let mut safe_site = void_site(
        &mut safe_targets,
        TestCallProtocol::NativeSafe {
            safepoint: 1,
            destination: scoop_lir::NativeSafeCallDestination::extern_function(c_call),
        },
        Vec::new(),
        Vec::new(),
    );
    let CallSite::NativeSafe(site) = &mut safe_site else {
        unreachable!()
    };
    site.roots = scoop_lir::NativeSafeRootSet::new(vec![scoop_lir::CallerRoot {
        source: scoop_lir::CallerRootSource::Param(0),
        scan: scoop_lir::NonEmptyRefScan::new(RefScan::References(vec![0])).unwrap(),
    }]);
    let mut safe_blocks = Arena::default();
    let safe_entry = safe_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![Instruction::Call { site: safe_site }],
        terminator: Terminator::Return {
            value: Some(Value::Param(0)),
        },
    });
    let safe = Function {
        callable_body: callable_body_at(file!(), line!()),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(vec![MANAGED_PTR], MANAGED_PTR),
        call_targets: safe_targets,
        locals: Arena::default(),
        temps: Arena::default(),
        blocks: safe_blocks,
        entry: safe_entry,
    };

    let mut borrowed_locals = Arena::default();
    let result_root = borrowed_locals.alloc(test_local("native_result_root", MANAGED_PTR));
    let mut borrowed_temps = Arena::default();
    let result = borrowed_temps.alloc(Temp { ty: MANAGED_PTR });
    let mut borrowed_targets = CallTargets::default();
    let mut borrowed_site = direct_site(
        &mut borrowed_targets,
        TestCallProtocol::NativeBorrowed {
            safepoint: 2,
            destination: scoop_lir::NativeBorrowedCallDestination::extern_function(borrowed),
            result: NativeBorrowedResultRoot::Rooted {
                storage: result_root,
            },
        },
        vec![MANAGED_PTR],
        (MANAGED_PTR, RefScan::References(vec![0])),
        result,
        vec![Value::Param(0)],
    );
    let CallSite::NativeBorrowed(site) = &mut borrowed_site else {
        unreachable!()
    };
    site.roots = scoop_lir::NativeBorrowedRootSet::new(vec![scoop_lir::CallerRoot {
        source: scoop_lir::CallerRootSource::Param(0),
        scan: scoop_lir::NonEmptyRefScan::new(RefScan::References(vec![0])).unwrap(),
    }]);
    let mut borrowed_blocks = Arena::default();
    let borrowed_entry = borrowed_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![Instruction::Call {
            site: borrowed_site,
        }],
        terminator: Terminator::Return {
            value: Some(Value::Temp(result)),
        },
    });
    let borrowed_function = Function {
        callable_body: callable_body_at(file!(), line!()),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(vec![MANAGED_PTR], MANAGED_PTR),
        call_targets: borrowed_targets,
        locals: borrowed_locals,
        temps: borrowed_temps,
        blocks: borrowed_blocks,
        entry: borrowed_entry,
    };

    let mut module = Module {
        release_hooks: Default::default(),
        cone: scoop_identity::ConeIdentity::SINGLE_FILE,
        globals: Arena::default(),
        initialization_units: Arena::default(),
        structs: scoop_lir::StructDefs::default(),
        enums: scoop_lir::EnumDefs::default(),
        extern_functions,
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_families: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![safe, borrowed_function],
        output: scoop_lir::LirOutput::Executable {
            entry: managed_function_ref(0),
        },
        meta: string_metadata(),
    };
    install_test_native_function_contract(&mut module, 1);
    refresh_module_safepoints(&mut module);

    let ir = ir_of(&module);
    assert!(ir.contains("@scoop_rt_push_caller_roots"));
    assert!(ir.contains("@scoop_rt_enter_native_safe"));
    assert!(ir.contains("@scoop_rt_leave_native_safe"));
    assert!(ir.contains("@scoop_rt_enter_native_borrowed"));
    assert!(ir.contains("@scoop_rt_leave_native_borrowed"));
    assert!(ir.contains("@scoop_rt_pop_caller_roots"));
    assert!(
        ir.contains("store ptr addrspace(1) null, ptr %native_result"),
        "managed native result storage must be zero before publication:\n{ir}"
    );
    let safe_leave = ir
        .find("call void @scoop_rt_leave_native_safe")
        .expect("safe transition leaves native state");
    assert!(
        ir[safe_leave..].contains("load ptr addrspace(1), ptr %managed_root_storage"),
        "published parameter roots must be reloaded from canonical storage after leave-native:\n{ir}"
    );
    let borrowed_call = ir
        .find("native_call = call ptr addrspace(1)")
        .expect("borrowed native call");
    let borrowed_enter = ir[..borrowed_call]
        .rfind("@scoop_rt_enter_native_borrowed")
        .expect("borrowed transition enters native state");
    assert!(
        ir[borrowed_enter..borrowed_call]
            .contains("load ptr addrspace(1), ptr %managed_root_storage"),
        "native arguments must be reloaded after the transition can park:\n{ir}"
    );
}

#[test]
fn continuation_state_atomics_keep_their_llvm_orderings() {
    let mut temps = Arena::default();
    let state_ty = LirType::MachineScalar(MachineScalarKind::CoroutineAdapterState);
    let loaded = temps.alloc(Temp {
        ty: state_ty.clone(),
    });
    let observed = temps.alloc(Temp {
        ty: state_ty.clone(),
    });
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::AtomicLoad {
                out: loaded,
                kind: MachineScalarKind::CoroutineAdapterState,
                object: Value::Param(0),
                offset: 16,
            },
            Instruction::AtomicStore {
                kind: MachineScalarKind::CoroutineAdapterState,
                object: Value::Param(0),
                offset: 16,
                value: Value::MachineScalar(MachineScalarValue::CoroutineAdapterState(
                    CoroutineAdapterState::CompletingSuccess,
                )),
            },
            Instruction::AtomicCompareExchange {
                out: observed,
                kind: MachineScalarKind::CoroutineAdapterState,
                object: Value::Param(0),
                offset: 16,
                expected: Value::Temp(loaded),
                replacement: Value::MachineScalar(MachineScalarValue::CoroutineAdapterState(
                    CoroutineAdapterState::Consumed,
                )),
            },
        ],
        terminator: Terminator::Return {
            value: Some(Value::Temp(observed)),
        },
    });
    let mut module = Module {
        release_hooks: Default::default(),
        cone: scoop_identity::ConeIdentity::SINGLE_FILE,
        globals: Arena::default(),
        initialization_units: Arena::default(),
        structs: scoop_lir::StructDefs::default(),
        enums: scoop_lir::EnumDefs::default(),
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_families: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![Function {
            callable_body: callable_body_at(file!(), line!()),
            safepoints: scoop_lir::SafepointIdentities::default(),
            gc_effect: GcEffect::Managed,
            signature: plain_scoop_signature(vec![MANAGED_PTR], state_ty),
            call_targets: CallTargets::default(),
            locals: Arena::default(),
            temps,
            blocks,
            entry,
        }],
        output: scoop_lir::LirOutput::Executable {
            entry: managed_function_ref(0),
        },
        meta: string_metadata(),
    };

    let ir = ir_of(&module);
    assert!(
        ir.contains("load atomic i64, ptr addrspace(1) %atomic_field_ptr acquire"),
        "continuation state reads must be acquire loads:\n{ir}"
    );
    assert!(
        ir.contains("store atomic i64 2, ptr addrspace(1) %atomic_field_ptr1 release"),
        "continuation state publication must be a release store:\n{ir}"
    );
    assert!(
        ir.contains("cmpxchg ptr addrspace(1) %atomic_field_ptr2")
            && ir.contains("acq_rel acquire"),
        "continuation state claims must be acq_rel/acquire compare-exchange:\n{ir}"
    );

    {
        let Instruction::AtomicLoad { offset, .. } =
            &mut module.functions[0].blocks[entry].instructions[0]
        else {
            unreachable!("test fixture starts with an atomic load")
        };
        *offset = 20;
    }
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("machine state atomics require 8-byte aligned fields");
    assert!(error.0.contains("not an aligned object field"), "{error}");

    {
        let Instruction::AtomicLoad { offset, kind, .. } =
            &mut module.functions[0].blocks[entry].instructions[0]
        else {
            unreachable!("test fixture starts with an atomic load")
        };
        *offset = 16;
        *kind = MachineScalarKind::ByteSize;
    }
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("non-state machine domains cannot use state atomics");
    assert!(
        error
            .0
            .contains("must produce its declared 64-bit machine state ByteSize"),
        "{error}"
    );
}
