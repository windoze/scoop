use super::*;
use scoop_lir::OptimizationMode;

/// A scalar HeapStore and ArraySet in the entry block, then a
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
    let mut alloc_site = direct_site(
        &mut call_targets,
        TestCallProtocol::Managed {
            safepoint: 1,
            destination: managed_runtime(scoop_lir::ManagedRuntimeFunction::Alloc),
        },
        vec![
            METADATA_PTR,
            LirType::MachineScalar(MachineScalarKind::ByteSize),
        ],
        (MANAGED_PTR, RefScan::References(vec![0])),
        t0,
        vec![
            Value::Param(0),
            Value::MachineScalar(MachineScalarValue::ByteSize(24)),
        ],
    );
    let live_array = || {
        statepoint_live(vec![statepoint_value(
            scoop_lir::CallerRootSource::Param(1),
            MANAGED_PTR,
            &[0],
        )])
    };
    set_managed_live(&mut alloc_site, live_array());
    let poll_signature = call_targets.void_signatures.alloc(VoidCallSignature::new(
        Vec::new(),
        scoop_lir::CallingConvention::Cdecl,
    ));
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
                    kind: scoop_lir::ManagedPollKind::Full,
                    target: poll_target,
                    safepoint: test_safepoint(2),
                    live: live_array(),
                },
            },
            Instruction::Call { site: alloc_site },
            Instruction::HeapStore {
                object: Value::Temp(t0),
                offset: 16,
                value: signed64(42),
            },
            Instruction::ArraySet {
                array: Value::Param(1),
                index: signed64(0),
                value: signed64(7),
                array_type: int_array,
            },
        ],
        terminator: Terminator::Br(header),
    };
    blocks[header] = BasicBlock {
        name: "while.cond".to_string(),
        instructions: vec![Instruction::ManagedPoll {
            site: scoop_lir::ManagedPollSite {
                kind: scoop_lir::ManagedPollKind::Full,
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
            signature: plain_scoop_signature(vec![METADATA_PTR, MANAGED_PTR], LirType::Void),
            call_targets,
            locals: Arena::default(),
            temps,
            blocks,
            entry,
        }],
        output: scoop_lir::LirOutput::Executable {
            entry: managed_function_ref(0),
        },
        meta,
    };
    refresh_module_safepoints(&mut module);
    module
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
    module.output = scoop_lir::LirOutput::Executable {
        entry: no_gc_function_ref(0),
    };
    for (_, block) in module.functions[0].blocks.iter_mut() {
        block.instructions.retain(|instruction| {
            !matches!(
                instruction,
                Instruction::ManagedPoll { .. }
                    | Instruction::Call { .. }
                    | Instruction::HeapStore { .. }
            )
        });
    }
    refresh_test_safepoints(&mut module.functions[0]);
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
fn scalar_stores_do_not_mark_cards() {
    let ir = ir_of(&barrier_module());
    assert!(!ir.contains("@scoop_gc_page_map"));
    assert!(!ir.contains("@scoop_rt_gc_write_barrier"));
}

#[test]
fn reference_store_marks_its_card() {
    let mut module = barrier_module();
    let function = &mut module.functions[0];
    let Instruction::HeapStore { value, .. } = &mut function.blocks[function.entry].instructions[2]
    else {
        unreachable!();
    };
    *value = Value::Param(1);
    let ir = ir_of(&module);
    assert!(
        ir.contains("@scoop_gc_page_map = external global [4096 x ptr]"),
        "sparse page map root missing:\n{ir}"
    );
    assert_eq!(
        ir.matches("load atomic ptr").count(),
        4,
        "four radix levels:\n{ir}"
    );
    for shift in [52, 40, 28, 16] {
        assert!(
            ir.contains(&format!(", {shift}")),
            "missing address level {shift}:\n{ir}"
        );
    }
    assert!(ir.contains("lshr i64"), "card index shift missing:\n{ir}");
    // Only the reference store marks a card; the integer array store does not.
    let marks = ir.matches("store atomic i8 1, ptr ").count();
    assert_eq!(marks, 1, "one card mark per reference store:\n{ir}");
    let mark = ir
        .lines()
        .find(|line| line.contains("store atomic i8 1, ptr "))
        .expect("card mark store");
    assert!(mark.contains("monotonic"), "relaxed card mark:\n{ir}");
    assert!(
        !ir.contains("atomicrmw"),
        "card mark must not read-modify-write:\n{ir}"
    );
}

fn reference_array_module(meta: LirMeta) -> Module {
    let mut module = barrier_module();
    module.meta = meta;
    let refs = array_type(
        &mut module.meta,
        "MutableArray<Object>",
        scoop_lir::ArrayKind::Mutable,
        MANAGED_PTR,
        8,
        8,
        RefScan::References(vec![0]),
    );
    let function = &mut module.functions[0];
    function.blocks[function.entry].instructions[3] = Instruction::ArraySet {
        array: Value::Param(1),
        index: signed64(0),
        value: Value::Param(1),
        array_type: refs,
    };
    module
}

#[test]
fn array_reference_store_mapping_strategy_is_target_specific() {
    for target in [
        scoop_target_data::DARWIN_AARCH64_ID,
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            let module = reference_array_module(string_metadata_for(
                scoop_target_data::layout(target).unwrap(),
            ));
            let profile = ValidatedBackendProfile::for_selection(
                scoop_target_data::selection(target).unwrap(),
            )
            .unwrap()
            .with_optimization(mode);
            let machine = profile.create_target_machine().unwrap();
            let context = Context::create();
            let llvm = emit_llvm_module(&context, &module, &machine, profile).unwrap();
            llvm.verify().unwrap();
            let ir = llvm.print_to_string().to_string();
            let direct = target == scoop_target_data::DARWIN_AARCH64_ID;
            assert_eq!(ir.contains("scoop.gc.array-region"), direct, "{ir}");
            assert_eq!(
                ir.matches("load atomic ptr").count(),
                if direct { 0 } else { 4 },
                "{ir}"
            );
        }
    }
}

#[test]
fn array_reference_store_selects_its_mapping_from_the_owner() {
    let module = reference_array_module(string_metadata());
    let initial = ir_of(&module);
    assert!(initial.contains("scoop.gc.array-region"));
    assert!(
        !initial.contains("array.owner_mapping"),
        "keep the mapping operation intact through loop optimization"
    );
    let ir = rewritten_ir_of(&module);
    assert!(
        !ir.contains("@scoop.gc.array.region"),
        "private query must be fully lowered:\n{ir}"
    );
    assert!(
        ir.contains("ptrtoint ptr addrspace(1)"),
        "retain managed owner provenance:\n{ir}"
    );
    assert!(
        ir.contains(", -16777216") && ir.contains("select i1"),
        "select the small or large mapping before reading its prefix:\n{ir}"
    );
    assert!(
        ir.contains("array.owner_mapping") && ir.contains("load ptr"),
        "immutable mapping owner:\n{ir}"
    );
    assert!(
        ir.contains("load atomic i64") && ir.contains("monotonic"),
        "read size bits atomically with concurrent pin state changes:\n{ir}"
    );
    assert!(
        !ir.contains("@scoop_gc_page_map"),
        "known array bases must not traverse the page map:\n{ir}"
    );
    assert_eq!(
        ir.matches("store atomic i8 1, ptr ").count(),
        1,
        "one card mark:\n{ir}"
    );
}

#[test]
fn array_mapping_identity_does_not_replace_an_ordinary_extern() {
    let context = Context::create();
    let llvm = context.create_module("array query identity");
    let builder = context.create_builder();
    let ty = context
        .ptr_type(AddressSpace::default())
        .fn_type(&[context.i64_type().into()], false);
    let ordinary = llvm.add_function("scoop.gc.array.region", ty, None);
    let caller = llvm.add_function("caller", ty, None);
    builder.position_at_end(context.append_basic_block(caller, "entry"));
    let address = caller.get_nth_param(0).unwrap().into_int_value();
    let external_call = builder
        .build_call(ordinary, &[address.into()], "external")
        .unwrap();
    assert!(!crate::gc_regions::is_array_query(
        external_call
            .try_as_basic_value()
            .basic()
            .unwrap()
            .as_instruction_value()
            .unwrap()
    ));
    let mapping = crate::gc_regions::array_mapping(&llvm, &builder, address).unwrap();
    builder.build_return(Some(&mapping)).unwrap();
    crate::gc_regions::inline_array_queries(&llvm, &host_target_machine().unwrap()).unwrap();
    llvm.verify().unwrap();
    assert_eq!(ordinary.count_basic_blocks(), 0);
    assert!(ordinary.as_global_value().get_first_use().is_some());
    assert!(!llvm.get_functions().any(|function| {
        function
            .get_basic_blocks()
            .iter()
            .flat_map(|block| block.get_instructions())
            .any(crate::gc_regions::is_array_query)
    }));
}

#[test]
fn heap_store_inside_the_object_header_is_rejected() {
    let mut module = barrier_module();
    let function = &mut module.functions[0];
    let entry = function.entry;
    function.blocks[entry].instructions[2] = Instruction::HeapStore {
        object: signed64(0),
        offset: 8,
        value: signed64(42),
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
