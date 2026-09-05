use super::*;

#[test]
fn managed_live_plan_produces_as1_relocation() {
    let leaves =
        scoop_lir::ManagedLeafPaths::new(vec![scoop_lir::ManagedLeafPath { byte_offset: 0 }])
            .unwrap();
    let live = scoop_lir::StatepointLiveSet::new(vec![scoop_lir::StatepointLiveValue {
        source: scoop_lir::CallerRootSource::Param(0),
        ty: MANAGED_PTR,
        leaves,
    }])
    .unwrap();
    let mut targets = CallTargets::default();
    let signature = targets.void_signatures.alloc(VoidCallSignature {
        params: Vec::new(),
        calling_convention: scoop_lir::CallingConvention::Cdecl,
    });
    let target = targets.managed_targets.void.alloc(scoop_lir::CallTarget {
        destination: scoop_lir::ManagedCallDestination::runtime(
            scoop_lir::ManagedRuntimeFunction::Safepoint,
        ),
        signature,
    });
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![Instruction::Call {
            site: CallSite::Managed(scoop_lir::ManagedCallSite {
                call: TypedCall::Void {
                    target,
                    args: Vec::new(),
                },
                safepoint: test_safepoint(1),
                live,
            }),
        }],
        terminator: Terminator::Return {
            value: Some(Value::Param(0)),
        },
    });
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
        functions: vec![Function {
            gc_effect: GcEffect::Managed,
            symbol: "scoop.live_root".to_string(),
            params: vec![MANAGED_PTR],
            return_ty: MANAGED_PTR,
            call_targets: targets,
            locals: Arena::default(),
            temps: Arena::default(),
            blocks,
            entry,
        }],
        entry_symbol: "scoop.live_root".to_string(),
        meta: string_metadata(),
    };
    let ir = rewritten_ir_of(&module);
    assert!(
        ir.contains("ptr addrspace(1)"),
        "managed AS1 is absent:\n{ir}"
    );
    assert!(
        ir.contains("@llvm.experimental.gc.relocate"),
        "relocation is absent:\n{ir}"
    );
}

#[test]
fn managed_invoke_uses_explicit_compiler_roots_without_exceptional_relocation() {
    let mut callee_blocks = Arena::default();
    let callee_entry = callee_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return {
            value: Some(Value::Param(0)),
        },
    });
    let callee = Function {
        gc_effect: GcEffect::Managed,
        symbol: "scoop.invoke_target".to_string(),
        params: vec![MANAGED_PTR],
        return_ty: MANAGED_PTR,
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps: Arena::default(),
        blocks: callee_blocks,
        entry: callee_entry,
    };

    let mut temps = Arena::default();
    let result = temps.alloc(Temp { ty: MANAGED_PTR });
    let record = temps.alloc(Temp {
        ty: LirType::ExceptionRecord,
    });
    let raw = temps.alloc(Temp { ty: RAW_PTR });
    let caught = temps.alloc(Temp { ty: MANAGED_PTR });
    let mut targets = CallTargets::default();
    let signature = targets.direct_signatures.alloc(DirectCallSignature {
        params: vec![MANAGED_PTR],
        result: MANAGED_PTR,
        result_scan: RefScan::References(vec![0]),
        calling_convention: scoop_lir::CallingConvention::Cdecl,
    });
    let target = targets.managed_targets.direct.alloc(scoop_lir::CallTarget {
        destination: managed_local(0),
        signature,
    });
    let mut blocks = Arena::default();
    let placeholder = |blocks: &mut Arena<BasicBlock>, name: &str| {
        blocks.alloc(BasicBlock {
            name: name.to_string(),
            instructions: Vec::new(),
            terminator: Terminator::Unreachable,
        })
    };
    let entry = placeholder(&mut blocks, "entry");
    let normal = placeholder(&mut blocks, "normal");
    let unwind = placeholder(&mut blocks, "unwind");
    blocks[entry] = BasicBlock {
        name: "entry".to_string(),
        instructions: vec![Instruction::Invoke {
            site: scoop_lir::InvokeSite::Managed(scoop_lir::ManagedInvokeSite {
                call: TypedCall::Direct {
                    target,
                    out: result,
                    args: vec![Value::Param(0)],
                },
                safepoint: test_safepoint(1),
                roots: scoop_lir::ExceptionalRootSet::new(vec![scoop_lir::ExceptionalRoot {
                    root: scoop_lir::CallerRoot {
                        source: scoop_lir::CallerRootSource::Param(0),
                        scan: scoop_lir::NonEmptyRefScan::new(RefScan::References(vec![0]))
                            .unwrap(),
                    },
                    normal_live: false,
                    unwind_live: true,
                }]),
                normal,
                unwind,
            }),
        }],
        terminator: Terminator::Br(normal),
    };
    blocks[normal] = BasicBlock {
        name: "normal".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return {
            value: Some(Value::Temp(result)),
        },
    };
    blocks[unwind] = BasicBlock {
        name: "unwind".to_string(),
        instructions: vec![
            Instruction::LandingPad { record, raw },
            Instruction::BeginCatch {
                out: caught,
                raw: Value::Temp(raw),
            },
            Instruction::EndCatch,
        ],
        terminator: Terminator::Return {
            value: Some(Value::Param(0)),
        },
    };
    let caller = Function {
        gc_effect: GcEffect::Managed,
        symbol: "scoop.invoke_caller".to_string(),
        params: vec![MANAGED_PTR],
        return_ty: MANAGED_PTR,
        call_targets: targets,
        locals: Arena::default(),
        temps,
        blocks,
        entry,
    };
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
        entry_symbol: "scoop.invoke_caller".to_string(),
        meta: string_metadata(),
    };

    let ir = rewritten_ir_of(&module);
    assert!(
        ir.contains("invoke token") && ir.contains("i64 1"),
        "managed invoke is not an explicit statepoint:\n{ir}"
    );
    assert!(
        !ir.contains("\"gc-live\"") && !ir.contains("@llvm.experimental.gc.relocate"),
        "managed invoke must not rely on exceptional relocation:\n{ir}"
    );
    assert!(
        ir.contains("@scoop_rt_push_compiler_roots")
            && ir.contains("@scoop_rt_pop_compiler_roots")
            && ir.contains("@scoop_rt_pop_top_compiler_roots"),
        "normal and unwind edges do not clean the compiler root frame:\n{ir}"
    );
}
