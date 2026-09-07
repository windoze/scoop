use super::*;

/// An M11-shaped module with both ordinary and suspend closure calls.
/// Both return aggregates so the machine ABI has a leading result slot;
/// the closure remains the first source-level argument and the suspend
/// call carries its continuation immediately after it.
fn closure_abi_module() -> Module {
    let ordinary_result_ty = LirType::Aggregate(vec![LirType::I64, MANAGED_PTR]);
    let suspend_result_ty = LirType::Aggregate(vec![LirType::I64, LirType::I64]);
    let aggregate_argument_ty = LirType::Aggregate(vec![LirType::I64, LirType::I64, LirType::I64]);
    let mut locals = Arena::default();
    let ordinary_result = locals.alloc(Local {
        name: "ordinary_result".to_string(),
        ty: ordinary_result_ty.clone(),
    });
    let suspend_result = locals.alloc(Local {
        name: "suspend_result".to_string(),
        ty: suspend_result_ty.clone(),
    });
    let aggregate_argument = locals.alloc(Local {
        name: "aggregate_argument".to_string(),
        ty: aggregate_argument_ty.clone(),
    });
    let aggregate_suspend_result = locals.alloc(Local {
        name: "aggregate_suspend_result".to_string(),
        ty: suspend_result_ty.clone(),
    });
    let mut call_targets = CallTargets::default();
    let ordinary_dispatch = closure_dispatch_destination(&mut call_targets, Value::Param(0), 2);
    let mut ordinary_site = indirect_result_site(
        &mut call_targets,
        TestCallProtocol::Managed {
            safepoint: 1,
            destination: ordinary_dispatch,
        },
        vec![MANAGED_PTR, LirType::I64],
        (ordinary_result_ty, RefScan::References(vec![8])),
        ordinary_result,
        vec![Value::Param(0), Value::Param(1)],
    );
    let suspend_dispatch = closure_dispatch_destination(&mut call_targets, Value::Param(0), 2);
    let mut suspend_site = indirect_result_site(
        &mut call_targets,
        TestCallProtocol::Managed {
            safepoint: 2,
            destination: suspend_dispatch,
        },
        vec![MANAGED_PTR, MANAGED_PTR],
        (suspend_result_ty.clone(), RefScan::None),
        suspend_result,
        vec![Value::Param(0), Value::Param(2)],
    );
    let aggregate_value =
        abi_value_with_layout(aggregate_argument_ty.clone(), 24, 8, RefScan::None);
    let aggregate_storage = scoop_lir::AbiArgumentStorage::new(
        aggregate_argument,
        &locals[aggregate_argument].ty,
        &aggregate_value,
    )
    .expect("aggregate closure argument storage has its exact ABI type");
    let managed_pointer = abi_value_with_layout(MANAGED_PTR, 8, 8, RefScan::References(vec![0]));
    let aggregate_suspend_signature =
        call_targets
            .indirect_result_signatures
            .alloc(IndirectResultCallSignature::scoop_sret(
                vec![
                    scoop_lir::AbiArgument::Direct(managed_pointer.clone()),
                    scoop_lir::AbiArgument::Indirect(aggregate_value),
                    scoop_lir::AbiArgument::Direct(managed_pointer),
                ],
                abi_value_with_layout(suspend_result_ty.clone(), 16, 8, RefScan::None),
                scoop_lir::CallingConvention::Cdecl,
            ));
    let aggregate_suspend_dispatch =
        closure_dispatch_destination(&mut call_targets, Value::Param(0), 2);
    let mut aggregate_suspend_site = protocol_site(
        &mut call_targets,
        TestCallProtocol::Managed {
            safepoint: 3,
            destination: aggregate_suspend_dispatch,
        },
        TestTypedCall::IndirectResult {
            signature: aggregate_suspend_signature,
            storage: aggregate_suspend_result,
            args: vec![
                scoop_lir::AbiCallArgument::Direct(Value::Param(0)),
                scoop_lir::AbiCallArgument::Indirect(aggregate_storage),
                scoop_lir::AbiCallArgument::Direct(Value::Param(2)),
            ],
        },
    );
    let closure_roots = || {
        statepoint_live(vec![
            statepoint_value(scoop_lir::CallerRootSource::Param(0), MANAGED_PTR, &[0]),
            statepoint_value(scoop_lir::CallerRootSource::Param(2), MANAGED_PTR, &[0]),
        ])
    };
    set_managed_live(&mut ordinary_site, closure_roots());
    set_managed_live(&mut suspend_site, closure_roots());
    set_managed_live(&mut aggregate_suspend_site, closure_roots());
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::Store {
                local: aggregate_argument,
                value: Value::Param(3),
            },
            Instruction::Call {
                site: ordinary_site,
            },
            Instruction::Call { site: suspend_site },
            Instruction::Call {
                site: aggregate_suspend_site,
            },
        ],
        terminator: Terminator::Return { value: None },
    });

    Module {
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
            gc_effect: GcEffect::Managed,
            symbol: "scoop.closure_abi".to_string(),
            signature: plain_scoop_signature(
                vec![
                    MANAGED_PTR,
                    LirType::I64,
                    MANAGED_PTR,
                    aggregate_argument_ty,
                ],
                LirType::Void,
            ),
            call_targets,
            locals,
            temps: Arena::default(),
            blocks,
            entry,
        }],
        entry_symbol: "scoop.closure_abi".to_string(),
        meta: string_metadata(),
    }
}

#[test]
fn closure_calls_preserve_hidden_abi_and_indirect_statepoints() {
    let module = closure_abi_module();
    let ir = ir_of(&module);
    assert_eq!(
        ir.lines()
            .filter(|line| {
                line.contains("getelementptr ptr, ptr addrspace(1)") && line.contains("i32 2")
            })
            .count(),
        3,
        "closure calls must load invoke from slot 2:\n{ir}"
    );
    assert!(
        ir.lines().any(|line| {
            line.contains("call void %dispatch_function")
                && line.contains(
                    "(ptr sret({ i64, ptr addrspace(1) }) align 8 %ordinary_result, ptr addrspace(1)",
                )
                && line.contains(", i64 %1)")
        }),
        "ordinary closure ABI must be (result slot, closure, arguments):\n{ir}"
    );
    assert!(
        ir.lines().any(|line| {
            line.contains("call void %dispatch_function")
                && line.contains("ptr sret({ i64, i64 }) align 8 %suspend_result, ptr addrspace(1)")
                && line.matches("ptr addrspace(1)").count() == 2
        }),
        "suspend closure ABI must keep continuation after the closure:\n{ir}"
    );
    assert!(
        ir.lines().any(|line| {
            line.contains("call void %dispatch_function")
                && line.contains(
                    "ptr sret({ i64, i64 }) align 8 %aggregate_suspend_result, ptr addrspace(1)",
                )
                && line.contains(
                    ", ptr byval({ i64, i64, i64 }) align 8 %aggregate_argument, ptr addrspace(1)",
                )
        }),
        "suspend closure ABI must keep aggregate arguments between the closure and continuation:\n{ir}"
    );
    assert!(
        ir.contains("%ordinary_result = alloca { i64, ptr addrspace(1) }")
            && ir.contains("%suspend_result = alloca { i64, i64 }"),
        "aggregate closure results must use typed return storage:\n{ir}"
    );

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let llvm = emit_llvm_module(&context, &module, &machine, host_profile()).expect("emit module");
    llvm.verify().expect("valid LLVM module");
    statepoint::rewrite(&llvm, &machine).expect("rewrite-statepoints-for-gc pass");
    let rewritten = llvm.print_to_string().to_string();
    assert_eq!(
        rewritten
            .lines()
            .filter(|line| {
                line.contains("call token")
                    && line.contains("gc.statepoint")
                    && line.contains("%dispatch_function")
            })
            .count(),
        3,
        "all managed indirect closure calls must become statepoints:\n{rewritten}"
    );
    assert!(
        rewritten.lines().any(|line| {
            line.contains("call token")
                && line.contains("gc.statepoint")
                && line.contains("%dispatch_function")
                && line.contains("sret({ i64, i64 }) align 8")
                && line.contains("byval({ i64, i64, i64 }) align 8")
        }),
        "RS4GC must retain aggregate closure argument attributes:\n{rewritten}"
    );
}

#[test]
fn elided_zst_calls_keep_logical_values_without_physical_abi_slots() {
    let zst = LirType::Aggregate(Vec::new());
    let mut callee_locals = Arena::default();
    let parameter_place = callee_locals.alloc(Local {
        name: "zst_parameter_place".to_string(),
        ty: zst.clone(),
    });
    let mut callee_temps = Arena::default();
    let parameter_address = callee_temps.alloc(Temp { ty: RAW_PTR });
    let mut callee_blocks = Arena::default();
    let callee_entry = callee_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::Store {
                local: parameter_place,
                value: Value::Param(0),
            },
            Instruction::LocalAddress {
                out: parameter_address,
                local: parameter_place,
            },
        ],
        terminator: Terminator::Return {
            value: Some(Value::Param(0)),
        },
    });
    let callee = Function {
        gc_effect: GcEffect::NoGc,
        symbol: "scoop.zst_identity".to_string(),
        signature: plain_scoop_signature(vec![zst.clone()], zst.clone()),
        call_targets: CallTargets::default(),
        locals: callee_locals,
        temps: callee_temps,
        blocks: callee_blocks,
        entry: callee_entry,
    };

    let mut caller_temps = Arena::default();
    let argument = caller_temps.alloc(Temp { ty: zst.clone() });
    let result = caller_temps.alloc(Temp { ty: zst.clone() });
    let mut call_targets = CallTargets::default();
    let site = elided_zst_site(
        &mut call_targets,
        TestCallProtocol::NoGc {
            destination: no_gc_local(0),
        },
        vec![zst.clone()],
        zst,
        result,
        vec![Value::Temp(argument)],
    );
    let mut caller_blocks = Arena::default();
    let caller_entry = caller_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::MakeAggregate {
                out: argument,
                elements: Vec::new(),
            },
            Instruction::Call { site },
        ],
        terminator: Terminator::Return { value: None },
    });
    let caller = Function {
        gc_effect: GcEffect::NoGc,
        symbol: "scoop.zst_caller".to_string(),
        signature: plain_scoop_signature(Vec::new(), LirType::Void),
        call_targets,
        locals: Arena::default(),
        temps: caller_temps,
        blocks: caller_blocks,
        entry: caller_entry,
    };
    assert_eq!(
        crate::module_context::instruction_temp_defs(&caller.blocks[caller.entry].instructions[1]),
        [Some(result), None],
        "an elided ZST call still defines its logical result temporary"
    );
    let module = Module {
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
        functions: vec![callee, caller],
        entry_symbol: "scoop.zst_caller".to_string(),
        meta: string_metadata(),
    };

    let ir = ir_of(&module);
    assert!(
        ir.contains("define void @scoop.zst_identity()")
            && ir.contains("call void @scoop.zst_identity()"),
        "elided ZST parameters and results must occupy no physical ABI slots:\n{ir}"
    );
    assert!(
        ir.contains("%zst_parameter_place = alloca i8, align 1"),
        "an address-taken elided parameter must retain an independent aligned place token:\n{ir}"
    );
}
