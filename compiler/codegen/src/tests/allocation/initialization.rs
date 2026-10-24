use super::*;

fn fixture(size: u64, scan: RefScan) -> (Module, TempId) {
    let mut module = heap_module();
    let descriptor = module
        .meta
        .type_descriptors
        .iter_mut()
        .find(|(_, descriptor)| descriptor.diagnostic_name == "Point")
        .unwrap()
        .1;
    descriptor.instance_shape =
        TypeInstanceShapeV1::fixed_object(module.meta.target_profile, size, 8, scan).unwrap();
    let function = &mut module.functions[1];
    let instructions = &mut function.blocks[function.entry].instructions;
    instructions.truncate(1);
    let Instruction::Call {
        site: CallSite::Managed(site),
    } = &mut instructions[0]
    else {
        panic!("the fixture starts with a managed allocation");
    };
    let TypedCall::Direct { out, args, .. } = &mut site.call else {
        panic!("the allocation returns a direct pointer");
    };
    args[1] = scoop_lir::AbiCallArgument::Direct(Value::MachineScalar(
        MachineScalarValue::ByteSize(size),
    ));
    let out = *out;
    (module, out)
}

fn check_clear(mut module: Module, bytes: u64) {
    refresh_module_safepoints(&mut module);
    let context = Context::create();
    let machine = host_target_machine().unwrap();
    let llvm = emit_llvm_module(
        &context,
        &module,
        &machine,
        host_profile().with_optimization(scoop_lir::OptimizationMode::Release),
    )
    .unwrap();
    llvm.verify().unwrap();
    let body = llvm
        .get_function(module.functions[1].symbol())
        .unwrap()
        .print_to_string()
        .to_string();
    let clears: Vec<_> = body
        .lines()
        .filter(|line| line.contains("@llvm.memset.p1.i64"))
        .collect();
    assert_eq!(clears.len(), 1, "{body}");
    assert!(
        clears[0].contains(&format!("i64 {bytes}, i1 false)")),
        "{body}"
    );
    assert!(body.contains("@scoop_runtime_alloc_slow"), "{body}");
}

fn store(object: Value, offset: u64) -> Instruction {
    Instruction::HeapStore {
        object,
        offset,
        value: signed64(offset),
    }
}

#[test]
fn initialization_follows_unconditional_blocks_and_local_snapshots() {
    let (mut module, out) = fixture(32, RefScan::None);
    let function = &mut module.functions[1];
    let first = function.locals.alloc(test_local("first", MANAGED_PTR));
    let second = function.locals.alloc(test_local("second", MANAGED_PTR));
    function.blocks[function.entry]
        .instructions
        .push(Instruction::Store {
            local: first,
            value: Value::Temp(out),
        });
    let next = function.blocks.alloc(BasicBlock {
        name: "initialize_copy".into(),
        instructions: vec![
            Instruction::Store {
                local: second,
                value: Value::Local(first),
            },
            Instruction::Store {
                local: first,
                value: Value::NullPointer(PointerKind::Managed),
            },
            store(Value::Local(second), 16),
            store(Value::Local(second), 24),
        ],
        terminator: Terminator::Return { value: None },
    });
    function.blocks[function.entry].terminator = Terminator::Br(next);
    check_clear(module, 16);
}

#[test]
fn initialization_preserves_reference_slots_and_scalar_tail_padding() {
    let (mut module, out) = fixture(32, RefScan::References(vec![16]));
    let function = &mut module.functions[1];
    function.blocks[function.entry].instructions.extend([
        Instruction::HeapStore {
            object: Value::Temp(out),
            offset: 16,
            value: Value::Temp(out),
        },
        store(Value::Temp(out), 24),
    ]);
    check_clear(module, 24);

    let (mut module, out) = fixture(24, RefScan::None);
    let function = &mut module.functions[1];
    function.blocks[function.entry]
        .instructions
        .push(Instruction::HeapStore {
            object: Value::Temp(out),
            offset: 16,
            value: Value::BoolConst(true),
        });
    check_clear(module, 24);
}

#[test]
fn initialization_counts_dense_structs_and_tuples_but_not_aggregate_padding() {
    for named in [false, true] {
        for padded in [false, true] {
            let (mut module, out) = fixture(32, RefScan::None);
            let fields = vec![
                if padded { LirType::I8 } else { LirType::I64 },
                LirType::I64,
            ];
            let ty = if named {
                let reference = module.structs.alloc_scoop(
                    test_exact_type("InitializationPayload"),
                    "InitializationPayload".into(),
                    16,
                    8,
                    false,
                    fields
                        .iter()
                        .enumerate()
                        .map(|(index, ty)| scoop_lir::StructField {
                            ty: ty.clone(),
                            layout: scoop_lir::FieldLayout {
                                offset: index as u64 * 8,
                                access_align: 8,
                            },
                        })
                        .collect(),
                );
                LirType::Struct(reference.definition())
            } else {
                LirType::Aggregate(fields)
            };
            let function = &mut module.functions[1];
            let payload = function.temps.alloc(Temp { ty });
            function.blocks[function.entry].instructions.extend([
                Instruction::MakeAggregate {
                    out: payload,
                    elements: vec![
                        if padded {
                            Value::IntegerConst(scoop_lir::LirIntegerConstant::Signed8(1))
                        } else {
                            signed64(1)
                        },
                        signed64(2),
                    ],
                },
                Instruction::HeapStore {
                    object: Value::Temp(out),
                    offset: 16,
                    value: Value::Temp(payload),
                },
            ]);
            check_clear(module, if padded { 32 } else { 16 });
        }
    }
}

#[test]
fn initialization_stops_at_reads_calls_and_addressed_local_storage() {
    for boundary in 0..3 {
        let (mut module, out) = fixture(32, RefScan::None);
        let descriptor = module
            .meta
            .type_descriptors
            .iter()
            .find(|(_, descriptor)| descriptor.diagnostic_name == "Point")
            .unwrap()
            .0;
        let function = &mut module.functions[1];
        let before = match boundary {
            0 => {
                let result = function.temps.alloc(Temp { ty: LirType::I64 });
                Instruction::HeapLoad {
                    out: result,
                    object: Value::Temp(out),
                    offset: 24,
                }
            }
            1 => {
                let result = function.temps.alloc(Temp { ty: LirType::I1 });
                Instruction::Call {
                    site: direct_site(
                        &mut function.call_targets,
                        TestCallProtocol::NoGc {
                            destination: no_gc_runtime(scoop_lir::NoGcRuntimeFunction::IsInstance),
                        },
                        vec![MANAGED_PTR, METADATA_PTR],
                        (LirType::I1, RefScan::None),
                        result,
                        vec![
                            Value::Temp(out),
                            Value::TypeDescriptor(TypeDescriptorRef::Local(descriptor)),
                        ],
                    ),
                }
            }
            _ => {
                let local = function.locals.alloc(test_local("addressed", MANAGED_PTR));
                let pointer = function.temps.alloc(Temp { ty: RAW_PTR });
                function.blocks[function.entry].instructions.insert(
                    0,
                    Instruction::LocalAddress {
                        out: pointer,
                        local,
                    },
                );
                Instruction::Store {
                    local,
                    value: Value::Temp(out),
                }
            }
        };
        function.blocks[function.entry].instructions.extend([
            before,
            store(Value::Temp(out), 16),
            store(Value::Temp(out), 24),
        ]);
        check_clear(module, 32);
    }
}

#[test]
fn initialization_does_not_assume_conditional_writes_execute() {
    let (mut module, out) = fixture(32, RefScan::None);
    module.output = scoop_lir::LirOutput::Library;
    let function = &mut module.functions[1];
    function.signature = plain_scoop_signature(vec![LirType::I1], LirType::Void);
    let written = function.blocks.alloc(BasicBlock {
        name: "written".into(),
        instructions: vec![store(Value::Temp(out), 24)],
        terminator: Terminator::Return { value: None },
    });
    let skipped = function.blocks.alloc(BasicBlock {
        name: "skipped".into(),
        instructions: Vec::new(),
        terminator: Terminator::Return { value: None },
    });
    function.blocks[function.entry].terminator = Terminator::CondBr {
        cond: Value::Param(0),
        then_block: written,
        else_block: skipped,
    };
    check_clear(module, 32);
}

#[test]
fn initialization_does_not_follow_reassigned_object_locals() {
    let (mut module, out) = fixture(32, RefScan::None);
    module.output = scoop_lir::LirOutput::Library;
    let function = &mut module.functions[1];
    function.signature = plain_scoop_signature(vec![MANAGED_PTR], LirType::Void);
    let local = function
        .locals
        .alloc(test_local("overwritten", MANAGED_PTR));
    function.blocks[function.entry].instructions.extend([
        Instruction::Store {
            local,
            value: Value::Temp(out),
        },
        Instruction::Store {
            local,
            value: Value::Param(0),
        },
        store(Value::Local(local), 24),
    ]);
    check_clear(module, 32);
}

#[test]
fn initialization_size_policy_keeps_large_x86_regions_fully_zeroed() {
    let arm64 = host_target_machine().unwrap().get_target().get_name() == c"aarch64";
    for size in [64, 72, 128] {
        let (mut module, out) = fixture(size, RefScan::None);
        let function = &mut module.functions[1];
        function.blocks[function.entry].instructions.extend(
            (16..size)
                .step_by(8)
                .map(|offset| store(Value::Temp(out), offset)),
        );
        check_clear(module, if arm64 || size == 64 { 16 } else { size });
    }
}
