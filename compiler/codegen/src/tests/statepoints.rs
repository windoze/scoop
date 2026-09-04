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
    let ir = ir_of(&values_module());
    assert_eq!(
        ir.match_indices("@scoop_td_String =").count(),
        1,
        "String must have exactly one descriptor definition"
    );
    assert!(ir.contains("@scoop_td_String ="));
}

#[test]
fn emits_complete_image_root_and_immortal_tables() {
    let mut module = values_module();
    module.globals.alloc(Global {
        symbol: "scoop.global.managed".to_string(),
        address_kind: PointerKind::Raw,
        scan: RefScan::References(vec![0]),
        init: GlobalInit::Storage {
            ty: MANAGED_PTR,
            initializer: ConstantValue::NullPointer(PointerKind::Managed),
            thread_local: false,
        },
    });

    let ir = ir_of(&module);
    assert!(
        ir.contains(
            "@scoop.global.managed.global_refs = private constant [2 x i64] [i64 1, i64 0]"
        ),
        "managed global scan is missing:\n{ir}"
    );
    assert!(
        ir.contains("@scoop_image_managed_globals = constant [1 x { ptr, ptr }]")
            && ir.contains("ptr @scoop.global.managed")
            && ir.contains("ptr @scoop.global.managed.global_refs"),
        "managed global descriptor table is incomplete:\n{ir}"
    );
    assert!(
        ir.contains("@scoop_image_managed_global_count = constant i64 1"),
        "managed global count is wrong:\n{ir}"
    );
    assert!(
        ir.contains("@scoop_image_immortal_objects = constant [2 x { ptr, i64, ptr }]")
            && ir.contains("ptr addrspacecast (ptr addrspace(1) @scoop.string.0 to ptr)")
            && ir.contains("ptr addrspacecast (ptr addrspace(1) @scoop.string.1 to ptr)")
            && ir.contains("ptr @scoop_td_String"),
        "immortal object descriptor table is incomplete:\n{ir}"
    );
    assert!(
        ir.contains("@scoop_image_immortal_object_count = constant i64 2"),
        "immortal object count is wrong:\n{ir}"
    );
}

#[test]
fn emits_addressable_zero_count_image_tables() {
    let module = enum_module();
    let ir = ir_of(&module);
    assert!(
        ir.contains("@scoop_image_managed_globals = constant [1 x { ptr, ptr }] zeroinitializer")
            && ir.contains("@scoop_image_managed_global_count = constant i64 0"),
        "empty managed-global table lacks its sentinel/count:\n{ir}"
    );
    assert!(
        ir.contains(
            "@scoop_image_immortal_objects = constant [1 x { ptr, i64, ptr }] zeroinitializer"
        ) && ir.contains("@scoop_image_immortal_object_count = constant i64 0"),
        "empty immortal table lacks its sentinel/count:\n{ir}"
    );
}

#[test]
fn managed_thread_local_global_is_rejected_at_codegen_boundary() {
    let mut module = values_module();
    module.globals.alloc(Global {
        symbol: "scoop.tls.managed".to_string(),
        address_kind: PointerKind::Raw,
        scan: RefScan::References(vec![0]),
        init: GlobalInit::Storage {
            ty: MANAGED_PTR,
            initializer: ConstantValue::NullPointer(PointerKind::Managed),
            thread_local: true,
        },
    });
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("managed TLS requires per-thread image-root registration");
    assert!(
        error
            .0
            .contains("thread-local global `@scoop.tls.managed` contains managed references"),
        "unexpected error: {error}"
    );
}

#[test]
fn typed_local_call_signature_cannot_be_replaced_by_a_callsite_guess() {
    let mut module = exceptions_module();
    module.functions[1].params.push(LirType::I64);
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("the local declaration and typed call target disagree");
    assert!(
        error.0.contains("disagrees with its existing declaration"),
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
        declaration: scoop_lir::ExternFunctionDeclaration {
            source_name: "wait".to_string(),
            native_symbol: "native_wait".to_string(),
            library: "fixture".to_string(),
            calling_convention: scoop_lir::CallingConvention::Cdecl,
            params: Vec::new(),
            return_type: LirType::Void,
        },
        bridge_symbol: "scoop_c_bridge_wait".to_string(),
        params: Vec::new(),
        return_type: scoop_lir::CType::Unit,
    });
    let borrowed = extern_functions.alloc_scoop(scoop_lir::ScoopExternFunction {
        declaration: scoop_lir::ExternFunctionDeclaration {
            source_name: "borrowed".to_string(),
            native_symbol: "native_borrowed".to_string(),
            library: "fixture".to_string(),
            calling_convention: scoop_lir::CallingConvention::Cdecl,
            params: vec![MANAGED_PTR],
            return_type: MANAGED_PTR,
        },
        gc_effect: GcEffect::Managed,
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
        gc_effect: GcEffect::Managed,
        symbol: "safe_root".to_string(),
        params: vec![MANAGED_PTR],
        return_ty: MANAGED_PTR,
        call_targets: safe_targets,
        locals: Arena::default(),
        temps: Arena::default(),
        blocks: safe_blocks,
        entry: safe_entry,
    };

    let mut borrowed_locals = Arena::default();
    let result_root = borrowed_locals.alloc(Local {
        name: "native_result_root".to_string(),
        ty: MANAGED_PTR,
    });
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
        gc_effect: GcEffect::Managed,
        symbol: "borrowed_result".to_string(),
        params: vec![MANAGED_PTR],
        return_ty: MANAGED_PTR,
        call_targets: borrowed_targets,
        locals: borrowed_locals,
        temps: borrowed_temps,
        blocks: borrowed_blocks,
        entry: borrowed_entry,
    };

    let module = Module {
        globals: Arena::default(),
        initialization_units: Arena::default(),
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions,
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![safe, borrowed_function],
        entry_symbol: "safe_root".to_string(),
        meta: string_metadata(),
    };

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
    let loaded = temps.alloc(Temp { ty: LirType::I64 });
    let observed = temps.alloc(Temp { ty: LirType::I64 });
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::AtomicLoad {
                out: loaded,
                object: Value::Param(0),
                offset: 16,
            },
            Instruction::AtomicStore {
                object: Value::Param(0),
                offset: 16,
                value: Value::IntConst(2),
            },
            Instruction::AtomicCompareExchange {
                out: observed,
                object: Value::Param(0),
                offset: 16,
                expected: Value::Temp(loaded),
                replacement: Value::IntConst(6),
            },
        ],
        terminator: Terminator::Return {
            value: Some(Value::Temp(observed)),
        },
    });
    let module = Module {
        globals: Arena::default(),
        initialization_units: Arena::default(),
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![Function {
            gc_effect: GcEffect::Managed,
            symbol: "continuation_atomics".to_string(),
            params: vec![MANAGED_PTR],
            return_ty: LirType::I64,
            call_targets: CallTargets::default(),
            locals: Arena::default(),
            temps,
            blocks,
            entry,
        }],
        entry_symbol: "continuation_atomics".to_string(),
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
}
