use super::*;

/// An M9-shaped module: a HeapStore and an ArraySet (both carry
/// the write-barrier card mark) in the entry block, then a
/// `while`-shaped loop (header ← body back edge) for the loop
/// safepoint poll.
fn barrier_module() -> Module {
    let mut meta = string_metadata();
    let int_array = array_type(
        &mut meta,
        "Array<Int>",
        scoop_lir::ArrayKind::Immutable,
        LirType::I64,
        8,
        8,
        RefScan::None,
    );
    let mut temps = Arena::default();
    let t0 = temps.alloc(Temp { ty: MANAGED_PTR }); // alloc result
    let mut call_targets = CallTargets::default();
    let alloc_site = direct_site(
        &mut call_targets,
        TestCallProtocol::Managed {
            safepoint: 1,
            destination: managed_runtime(scoop_lir::ManagedRuntimeFunction::Alloc),
        },
        vec![METADATA_PTR, LirType::I64],
        (MANAGED_PTR, RefScan::References(vec![0])),
        t0,
        vec![Value::Param(0), Value::IntConst(24)],
    );
    let poll_signature = call_targets.void_signatures.alloc(VoidCallSignature {
        params: Vec::new(),
        calling_convention: scoop_lir::CallingConvention::Cdecl,
    });
    let poll_target = call_targets
        .managed_targets
        .void
        .alloc(scoop_lir::CallTarget {
            destination: scoop_lir::ManagedCallDestination::runtime(
                scoop_lir::ManagedRuntimeFunction::Safepoint,
            ),
            signature: poll_signature,
        });
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![],
        terminator: Terminator::Unreachable, // placeholder, filled below
    });
    let header = blocks.alloc(BasicBlock {
        name: "while.cond".to_string(),
        instructions: vec![],
        terminator: Terminator::Unreachable,
    });
    let body = blocks.alloc(BasicBlock {
        name: "while.body".to_string(),
        instructions: vec![],
        terminator: Terminator::Unreachable,
    });
    let exit = blocks.alloc(BasicBlock {
        name: "while.exit".to_string(),
        instructions: vec![],
        terminator: Terminator::Return { value: None },
    });
    blocks[entry] = BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::ManagedPoll {
                site: scoop_lir::ManagedPollSite {
                    target: poll_target,
                    safepoint: test_safepoint(2),
                    live: scoop_lir::StatepointLiveSet::default(),
                },
            },
            Instruction::Call { site: alloc_site },
            Instruction::HeapStore {
                object: Value::Temp(t0),
                offset: 16,
                value: Value::IntConst(42),
            },
            Instruction::ArraySet {
                array: Value::Param(1),
                index: Value::IntConst(0),
                value: Value::IntConst(7),
                array_type: int_array,
            },
        ],
        terminator: Terminator::Br(header),
    };
    blocks[header] = BasicBlock {
        name: "while.cond".to_string(),
        instructions: vec![Instruction::ManagedPoll {
            site: scoop_lir::ManagedPollSite {
                target: poll_target,
                safepoint: test_safepoint(3),
                live: scoop_lir::StatepointLiveSet::default(),
            },
        }],
        terminator: Terminator::CondBr {
            cond: Value::BoolConst(true),
            then_block: body,
            else_block: exit,
        },
    };
    blocks[body] = BasicBlock {
        name: "while.body".to_string(),
        instructions: vec![],
        terminator: Terminator::Br(header),
    };
    Module {
        globals: Arena::default(),
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![Function {
            gc_effect: GcEffect::Managed,
            symbol: "scoop_main".to_string(),
            params: vec![METADATA_PTR, MANAGED_PTR],
            return_ty: LirType::Void,
            call_targets,
            locals: Arena::default(),
            temps,
            blocks,
            entry,
        }],
        entry_symbol: "scoop_main".to_string(),
        meta,
    }
}

#[test]
fn functions_carry_the_gc_strategy_and_poll_safepoints() {
    let ir = ir_of(&barrier_module());
    assert!(
        ir.contains("gc \"statepoint-example\""),
        "function lacks the GC strategy:\n{ir}"
    );
    // One poll at the function entry, one at the loop header.
    let polls = ir.matches("call void @scoop_rt_safepoint()").count();
    assert_eq!(polls, 2, "entry + loop-header safepoint polls:\n{ir}");
}

#[test]
fn no_gc_functions_carry_neither_gc_strategy_nor_safepoint_polls() {
    let mut module = barrier_module();
    module.functions[0].gc_effect = GcEffect::NoGc;
    for (_, block) in module.functions[0].blocks.iter_mut() {
        block
            .instructions
            .retain(|instruction| !matches!(instruction, Instruction::ManagedPoll { .. }));
    }
    let ir = ir_of(&module);
    assert!(
        !ir.contains("gc \"statepoint-example\""),
        "NoGC function unexpectedly carries the GC strategy:\n{ir}"
    );
    assert_eq!(
        ir.matches("call void @scoop_rt_safepoint()").count(),
        0,
        "NoGC function unexpectedly polls safepoints:\n{ir}"
    );
}

#[test]
fn heap_stores_mark_the_write_barrier_card() {
    let ir = ir_of(&barrier_module());
    // The card table is a pointer variable: load the (pre-biased)
    // base, then GEP by the card index.
    assert!(
        ir.contains("@scoop_gc_card_table = external global ptr"),
        "card table pointer global missing:\n{ir}"
    );
    assert!(
        ir.contains("load ptr, ptr @scoop_gc_card_table"),
        "card table base load missing:\n{ir}"
    );
    assert!(ir.contains("lshr i64"), "card index shift missing:\n{ir}");
    // One monotonic atomic card mark per heap store: the HeapStore and
    // the ArraySet element store.
    let marks = ir.matches(" = atomicrmw or ptr ").count();
    assert_eq!(marks, 2, "one card mark per heap store:\n{ir}");
}

#[test]
fn heap_store_inside_the_object_header_is_rejected() {
    let mut module = barrier_module();
    let function = &mut module.functions[0];
    let entry = function.entry;
    function.blocks[entry].instructions[2] = Instruction::HeapStore {
        object: Value::IntConst(0),
        offset: 8,
        value: Value::IntConst(42),
    };
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("offset 8 is inside the object header, not a field");
    assert!(
        error.0.contains("object header"),
        "unexpected error: {error}"
    );
}

fn stackmap_qualification_module() -> Module {
    fn poll_function(
        symbol: &str,
        safepoint: u64,
        params: Vec<LirType>,
        return_ty: LirType,
        live: scoop_lir::StatepointLiveSet,
        return_value: Option<Value>,
    ) -> Function {
        let mut call_targets = CallTargets::default();
        let signature = call_targets.void_signatures.alloc(VoidCallSignature {
            params: Vec::new(),
            calling_convention: scoop_lir::CallingConvention::Cdecl,
        });
        let target = call_targets
            .managed_targets
            .void
            .alloc(scoop_lir::CallTarget {
                destination: scoop_lir::ManagedCallDestination::runtime(
                    scoop_lir::ManagedRuntimeFunction::Safepoint,
                ),
                signature,
            });
        let mut blocks = Arena::default();
        let entry = blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![Instruction::ManagedPoll {
                site: scoop_lir::ManagedPollSite {
                    target,
                    safepoint: test_safepoint(safepoint),
                    live,
                },
            }],
            terminator: Terminator::Return {
                value: return_value,
            },
        });
        Function {
            gc_effect: GcEffect::Managed,
            symbol: symbol.to_string(),
            params,
            return_ty,
            call_targets,
            locals: Arena::default(),
            temps: Arena::default(),
            blocks,
            entry,
        }
    }

    let pair = LirType::Aggregate(vec![MANAGED_PTR, MANAGED_PTR]);
    Module {
        globals: Arena::default(),
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![
            poll_function(
                "scoop_qualification_zero",
                1,
                Vec::new(),
                LirType::Void,
                scoop_lir::StatepointLiveSet::default(),
                None,
            ),
            poll_function(
                "scoop_qualification_one",
                2,
                vec![MANAGED_PTR],
                MANAGED_PTR,
                statepoint_live(vec![statepoint_value(
                    scoop_lir::CallerRootSource::Param(0),
                    MANAGED_PTR,
                    &[0],
                )]),
                Some(Value::Param(0)),
            ),
            poll_function(
                "scoop_qualification_many",
                3,
                vec![pair.clone()],
                pair.clone(),
                statepoint_live(vec![statepoint_value(
                    scoop_lir::CallerRootSource::Param(0),
                    pair,
                    &[0, 8],
                )]),
                Some(Value::Param(0)),
            ),
        ],
        entry_symbol: "scoop_qualification_zero".to_string(),
        meta: string_metadata(),
    }
}

fn assert_aarch64_frame_disassembly(object: &std::path::Path, optimization: &str) {
    let output = std::process::Command::new("otool")
        .arg("-tvV")
        .arg(object)
        .output()
        .expect("disassemble qualification object");
    assert!(
        output.status.success(),
        "{optimization}: otool failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let disassembly = String::from_utf8_lossy(&output.stdout);
    for symbol in [
        "_scoop_qualification_zero:",
        "_scoop_qualification_one:",
        "_scoop_qualification_many:",
    ] {
        let start = disassembly
            .find(symbol)
            .unwrap_or_else(|| panic!("{optimization}: missing {symbol}:\n{disassembly}"));
        let body = &disassembly[start..];
        let end = body[1..]
            .find("\n_")
            .map_or(body.len(), |offset| offset + 1);
        let body = &body[..end];
        assert!(
            body.contains("stp\tx29, x30, [sp"),
            "{optimization}: {symbol} does not save the frame record:\n{body}"
        );
        assert!(
            body.contains("add\tx29, sp") || body.contains("mov\tx29, sp"),
            "{optimization}: {symbol} does not establish the x29 frame chain:\n{body}"
        );
        assert!(
            body.lines().any(|line| line.contains("\tbl\t")),
            "{optimization}: {symbol} has no managed call instruction:\n{body}"
        );
    }
}

#[test]
fn aarch64_statepoint_artifacts_are_qualified_at_o0_and_o2() {
    let module = stackmap_qualification_module();
    let expected = statepoint::expectations(&module).expect("complete safepoint manifest");
    assert_eq!(expected.root_count(1), Some(0));
    assert_eq!(expected.root_count(2), Some(1));
    assert_eq!(expected.root_count(3), Some(2));
    let profile = host_profile();
    for (name, optimization) in [
        ("o0", OptimizationLevel::None),
        ("o2", OptimizationLevel::Default),
    ] {
        let machine = profile
            .create_qualification_target_machine(optimization)
            .expect("qualified target machine");
        let context = Context::create();
        let llvm = emit_llvm_module(&context, &module, &machine, profile).expect("emit module");
        llvm.verify().expect("valid LLVM module");
        statepoint::rewrite(&llvm, &machine).expect("rewrite-statepoints-for-gc pass");
        llvm.verify().expect("valid post-RS4GC module");
        statepoint::verify_rewritten(&llvm, &expected, profile)
            .expect("statepoint manifest matches");
        let ir = llvm.print_to_string().to_string();
        assert!(
            ir.contains("gc.statepoint"),
            "{name}: statepoint intrinsics missing after rewrite-statepoints-for-gc:\n{ir}"
        );

        let output = std::env::temp_dir().join(format!(
            "scoop_codegen_statepoint_{name}_{}.o",
            std::process::id()
        ));
        machine
            .write_to_file(&llvm, FileType::Object, &output)
            .expect("write object");
        profile
            .verify_object(&output, &expected)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_aarch64_frame_disassembly(&output, name);
        std::fs::remove_file(&output).ok();
    }
}

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
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
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
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
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

#[test]
fn type_descriptors_carry_the_gc_scan_descriptors() {
    // A class's plain table is count-prefixed (`[N, off0, ..]`,
    // runtime/include/scoop_rt.h's M9 scan-descriptor contract).
    let ir = ir_of(&heap_module());
    assert!(
        ir.contains("@scoop_td_Point.refs = private constant [2 x i64] [i64 1, i64 24]"),
        "plain scan table must be count-prefixed:\n{ir}"
    );

    // A reference-element array's TD carries the SCOOP_REFS_ARRAY
    // sentinel (u64::MAX, printed -1), its stride, and a pointer
    // to the recursive scan for one inline element.
    let nested_element_scan = RefScan::Sequence(vec![
        RefScan::References(vec![16]),
        RefScan::References(vec![8]),
    ]);
    let mut meta = string_metadata();
    let ref_array_type = array_type(
        &mut meta,
        "ArrayRef",
        scoop_lir::ArrayKind::Immutable,
        MANAGED_PTR,
        8,
        8,
        RefScan::References(vec![0]),
    );
    let nested_array_type = array_type(
        &mut meta,
        "ArrayNested",
        scoop_lir::ArrayKind::Immutable,
        LirType::Aggregate(vec![LirType::I64, MANAGED_PTR, MANAGED_PTR]),
        24,
        8,
        nested_element_scan,
    );
    let mut temps = Arena::default();
    let array = temps.alloc(Temp { ty: MANAGED_PTR });
    let nested_array = temps.alloc(Temp { ty: MANAGED_PTR });
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::ArrayAlloc {
                out: array,
                elements: vec![],
                array_type: ref_array_type,
                safepoint: test_safepoint(1),
                live: scoop_lir::StatepointLiveSet::default(),
            },
            Instruction::ArrayAlloc {
                out: nested_array,
                elements: vec![],
                array_type: nested_array_type,
                safepoint: test_safepoint(2),
                live: scoop_lir::StatepointLiveSet::default(),
            },
        ],
        terminator: Terminator::Return { value: None },
    });
    let runtime_type_id = meta.type_descriptors.len() as u64 + 1;
    meta.type_descriptors.alloc(TypeDescriptor {
        name: "Holder".to_string(),
        symbol: "scoop_td_Holder".to_string(),
        runtime_type_id,
        size: 56,
        align: 8,
        scan: TypeDescriptorScan::Fixed(RefScan::Sequence(vec![
            RefScan::References(vec![16]),
            RefScan::References(vec![40, 48]),
        ])),
        parent: None,
        vtable: vec![],
        itables: vec![],
    });
    let module = Module {
        globals: Arena::default(),
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![Function {
            gc_effect: GcEffect::Managed,
            symbol: "scoop_main".to_string(),
            params: vec![],
            return_ty: LirType::Void,
            call_targets: CallTargets::default(),
            locals: Arena::default(),
            temps,
            blocks,
            entry,
        }],
        entry_symbol: "scoop_main".to_string(),
        meta,
    };
    let ir = ir_of(&module);
    assert!(
            ir.contains(
                "@scoop_td_ArrayRef.element = private constant [2 x i64] [i64 1, i64 0]"
            ) && ir.contains(
                "@scoop_td_ArrayRef.refs = private constant [3 x i64] [i64 -1, i64 8, i64 ptrtoint (ptr @scoop_td_ArrayRef.element to i64)]"
            ),
            "reference-element array TD must carry SCOOP_REFS_ARRAY:\n{ir}"
        );
    assert!(
            ir.contains(
                "@scoop_td_ArrayNested.element.part.1 = private constant [2 x i64] [i64 1, i64 8]"
            ) && ir.contains(
                "@scoop_td_ArrayNested.element = private constant [4 x i64] [i64 -2, i64 2"
            ) && ir.contains(
                "@scoop_td_ArrayNested.refs = private constant [3 x i64] [i64 -1, i64 24, i64 ptrtoint (ptr @scoop_td_ArrayNested.element to i64)]"
            ),
            "aggregate array TD must wrap the recursive element scan:\n{ir}"
        );
    assert!(
        ir.contains(
            "@scoop_td_Holder.refs.part.1 = private constant [3 x i64] [i64 2, i64 40, i64 48]"
        ),
        "nested tagged enum scan must use fixed ref offsets:\n{ir}"
    );
    assert!(
        ir.contains("@scoop_td_Holder.refs = private constant [4 x i64] [i64 -2, i64 2"),
        "aggregate scan must compose fixed scans:\n{ir}"
    );
}
