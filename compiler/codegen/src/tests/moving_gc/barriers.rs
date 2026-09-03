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
