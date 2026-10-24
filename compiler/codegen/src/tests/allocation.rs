use super::*;
use inkwell::values::AnyValue;

fn allocation_ir(
    size: u64,
    dynamic: bool,
    known_descriptor: bool,
    mode: scoop_lir::OptimizationMode,
) -> String {
    let mut module = heap_module();
    let descriptor = module
        .meta
        .type_descriptors
        .iter_mut()
        .find(|(_, descriptor)| descriptor.diagnostic_name == "Point")
        .unwrap()
        .1;
    descriptor.instance_shape = TypeInstanceShapeV1::fixed_object(
        module.meta.target_profile,
        size,
        8,
        RefScan::References(vec![24]),
    )
    .unwrap();
    let function = &mut module.functions[1];
    let constant = Value::MachineScalar(MachineScalarValue::ByteSize(size));
    let value = if dynamic {
        let local = function.locals.alloc(test_local(
            "requested_size",
            LirType::MachineScalar(MachineScalarKind::ByteSize),
        ));
        function.blocks[function.entry].instructions.insert(
            0,
            Instruction::Store {
                local,
                value: constant,
            },
        );
        Value::Local(local)
    } else {
        constant
    };
    let descriptor_local = (!known_descriptor).then(|| {
        function
            .locals
            .alloc(test_local("allocation_descriptor", METADATA_PTR))
    });
    let mut descriptor_value = None;
    let mut replaced = false;
    for instruction in &mut function.blocks[function.entry].instructions {
        let Instruction::Call {
            site: CallSite::Managed(site),
        } = instruction
        else {
            continue;
        };
        let TypedCall::Direct { args, .. } = &mut site.call else {
            continue;
        };
        if matches!(
            args.get(1),
            Some(scoop_lir::AbiCallArgument::Direct(Value::MachineScalar(
                MachineScalarValue::ByteSize(32)
            )))
        ) {
            args[1] = scoop_lir::AbiCallArgument::Direct(value);
            if let Some(local) = descriptor_local {
                let scoop_lir::AbiCallArgument::Direct(value) = args[0] else {
                    panic!("the allocation descriptor is a direct metadata pointer");
                };
                descriptor_value = Some((local, value));
                args[0] = scoop_lir::AbiCallArgument::Direct(Value::Local(local));
            }
            replaced = true;
        }
    }
    assert!(replaced, "the heap fixture must contain its allocation");
    if let Some((local, value)) = descriptor_value {
        function.blocks[function.entry]
            .instructions
            .insert(0, Instruction::Store { local, value });
    }
    let context = Context::create();
    let machine = host_target_machine().unwrap();
    let llvm = emit_llvm_module(
        &context,
        &module,
        &machine,
        host_profile().with_optimization(mode),
    )
    .unwrap();
    llvm.verify()
        .expect("cross-word completion must be a valid allocation phi predecessor");
    llvm.print_to_string().to_string()
}

#[test]
fn small_release_allocations_publish_ordered_metadata_inline() {
    for size in [32, 128] {
        let ir = allocation_ir(size, false, true, scoop_lir::OptimizationMode::Release);
        assert!(!ir.contains("@scoop_runtime_finish_tlab_alloc"), "{ir}");
        assert!(ir.contains("@scoop_runtime_alloc_slow"), "{ir}");
        assert!(ir.contains("@llvm.memset.p1.i64"), "{ir}");
        assert!(!ir.contains("allocation_alignment = load"), "{ir}");
        assert!(!ir.contains("allocation_hook = load"), "{ir}");
        assert!(ir.contains(&format!("i64 {size}, i1 false")), "{ir}");
        assert!(
            ir.contains("load atomic i64") && ir.contains(" monotonic, align 8"),
            "{ir}"
        );
        assert!(
            ir.contains("store atomic i64") && ir.contains(" release, align 8"),
            "{ir}"
        );
        assert!(!ir.contains("atomicrmw"), "{ir}");
        assert!(ir.contains("%alloc.lines.done.0"), "{ir}");
    }
}

#[test]
fn debug_large_dynamic_and_unknown_allocations_keep_the_shared_finish_entry() {
    for (size, dynamic, known_descriptor, mode) in [
        (32, false, true, scoop_lir::OptimizationMode::Debug),
        (136, false, true, scoop_lir::OptimizationMode::Release),
        (32, true, true, scoop_lir::OptimizationMode::Release),
        (32, false, false, scoop_lir::OptimizationMode::Release),
    ] {
        let ir = allocation_ir(size, dynamic, known_descriptor, mode);
        assert!(
            ir.contains("call void @scoop_runtime_finish_tlab_alloc"),
            "{ir}"
        );
        assert!(
            ir.contains(
                "declare void @scoop_runtime_finish_tlab_alloc(ptr addrspace(1), ptr, i64)"
            ),
            "{ir}"
        );
        assert!(!ir.contains("alloc.lines.second"), "{ir}");
        assert!(ir.contains("{ ptr, ptr, ptr, ptr, ptr, ptr, ptr }"), "{ir}");
        if !known_descriptor || mode == scoop_lir::OptimizationMode::Debug {
            assert!(ir.contains("allocation_alignment = load"), "{ir}");
            assert!(ir.contains("allocation_hook = load"), "{ir}");
        }
    }
}

#[test]
fn statically_large_allocations_do_not_cache_a_tlab_context() {
    let ir = allocation_ir(32768, false, true, scoop_lir::OptimizationMode::Release);
    assert!(ir.contains("@scoop_runtime_alloc_slow"), "{ir}");
    if host_target_machine().unwrap().get_target().get_name() == c"aarch64" {
        assert!(!ir.contains("@scoop_rt_allocation_context"), "{ir}");
    }
    assert!(!ir.contains("allocation_context_cache"), "{ir}");
}

#[test]
fn allocation_context_reuse_does_not_load_tls_on_the_no_allocation_path() {
    let mut module = heap_module();
    let function = &mut module.functions[1];
    function.signature = plain_scoop_signature(vec![LirType::I1], LirType::Void);
    let skip = function.blocks.alloc(BasicBlock {
        name: "skip_allocation".into(),
        instructions: Vec::new(),
        terminator: Terminator::Return { value: None },
    });
    function.entry = function.blocks.alloc(BasicBlock {
        name: "allocation_guard".into(),
        instructions: Vec::new(),
        terminator: Terminator::CondBr {
            cond: Value::Param(0),
            then_block: function.entry,
            else_block: skip,
        },
    });
    let symbol = function.symbol().to_owned();
    for mode in [
        scoop_lir::OptimizationMode::Debug,
        scoop_lir::OptimizationMode::Release,
    ] {
        let context = Context::create();
        let machine = host_target_machine().unwrap();
        let reuse_context = mode == scoop_lir::OptimizationMode::Release
            && machine.get_target().get_name() == c"aarch64";
        let llvm = emit_llvm_module(
            &context,
            &module,
            &machine,
            host_profile().with_optimization(mode),
        )
        .unwrap();
        llvm.verify().unwrap();
        let body = llvm.get_function(&symbol).unwrap();
        for block in body.get_basic_blocks() {
            if matches!(
                block.get_name().to_str().unwrap(),
                "allocation_guard" | "skip_allocation"
            ) {
                assert!(block.get_instructions().all(|instruction| {
                    !instruction
                        .print_to_string()
                        .to_string()
                        .contains("@scoop_rt_allocation_context")
                }));
            }
        }
        let ir = body.print_to_string().to_string();
        if reuse_context {
            assert!(
                ir.contains("store ptr null, ptr %allocation_context_cache"),
                "{ir}"
            );
            assert!(ir.contains("allocation_context_value = phi ptr"), "{ir}");
            assert!(ir.contains("alloc.context.load.0"), "{ir}");
            assert!(
                ir.contains("allocation_context = load volatile ptr"),
                "{ir}"
            );
        } else {
            assert!(!ir.contains("allocation_context_cache"), "{ir}");
        }
    }
}
