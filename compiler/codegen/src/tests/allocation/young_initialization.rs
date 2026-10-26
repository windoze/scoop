use super::*;

fn fixture(size: u64, references: Vec<u64>) -> (Module, TempId) {
    let (mut module, out) = super::initialization::fixture(size, RefScan::References(references));
    module.output = scoop_lir::LirOutput::Library;
    let function = &mut module.functions[1];
    function.signature = plain_scoop_signature(vec![MANAGED_PTR], LirType::Void);
    let Instruction::Call { site } = &mut function.blocks[function.entry].instructions[0] else {
        unreachable!()
    };
    set_managed_live(
        site,
        statepoint_live(vec![statepoint_value(
            scoop_lir::CallerRootSource::Param(0),
            MANAGED_PTR,
            &[0],
        )]),
    );
    (module, out)
}

fn body(module: &mut Module, mode: scoop_lir::OptimizationMode) -> String {
    refresh_module_safepoints(module);
    let context = Context::create();
    let machine = host_target_machine().unwrap();
    let llvm = emit_llvm_module(
        &context,
        module,
        &machine,
        host_profile().with_optimization(mode),
    )
    .unwrap();
    llvm.verify().unwrap();
    llvm.get_function(module.functions[1].symbol())
        .unwrap()
        .print_to_string()
        .to_string()
}

fn store(object: Value) -> Instruction {
    Instruction::HeapStore {
        object,
        offset: 16,
        value: Value::Param(0),
    }
}

fn guards(ir: &str) -> usize {
    ir.lines()
        .filter(|line| line.contains("br i1") && line.contains("label %init.barrier"))
        .count()
}

#[test]
fn reference_initialization_uses_the_actual_tlab_choice() {
    let (mut module, out) = fixture(24, vec![16]);
    let function = &mut module.functions[1];
    function.blocks[function.entry]
        .instructions
        .push(store(Value::Temp(out)));
    let ir = body(&mut module, scoop_lir::OptimizationMode::Release);
    let choice = ir
        .lines()
        .find(|line| line.contains("br i1") && line.contains("label %alloc.fast"))
        .unwrap()
        .split(", label")
        .next()
        .unwrap()
        .trim();
    assert!(
        ir.contains(&format!(
            "{choice}, label %init.continue, label %init.barrier"
        )),
        "{ir}"
    );
    let (before, barrier) = ir.split_once("init.barrier:").unwrap();
    assert!(!before.contains("@scoop_gc_page_map"), "{ir}");
    assert!(barrier.contains("@scoop_gc_page_map"), "{ir}");
    assert!(before.contains("@scoop_runtime_alloc_slow"), "{ir}");
    assert!(before.contains("store ptr addrspace(1)"), "{ir}");

    let debug = body(&mut module, scoop_lir::OptimizationMode::Debug);
    assert_eq!(guards(&debug), 0, "{debug}");
    assert!(debug.contains("@scoop_gc_page_map"), "{debug}");
}

#[test]
fn reference_initialization_keeps_copy_snapshots_and_reassignment() {
    let (mut module, out) = fixture(24, vec![16]);
    let function = &mut module.functions[1];
    let first = function.locals.alloc(test_local("first", MANAGED_PTR));
    let copy = function.locals.alloc(test_local("copy", MANAGED_PTR));
    function.blocks[function.entry].instructions.extend([
        Instruction::Store {
            local: first,
            value: Value::Temp(out),
        },
        Instruction::Store {
            local: copy,
            value: Value::Local(first),
        },
        Instruction::Store {
            local: first,
            value: Value::Param(0),
        },
        store(Value::Local(copy)),
        store(Value::Local(first)),
    ]);
    let ir = body(&mut module, scoop_lir::OptimizationMode::Release);
    assert_eq!(guards(&ir), 1, "{ir}");
    assert_eq!(ir.matches("@scoop_gc_page_map").count(), 2, "{ir}");
}

#[test]
fn reference_initialization_stops_at_observation_address_and_call() {
    for boundary in 0..3 {
        let (mut module, out) = fixture(24, vec![16]);
        let function = &mut module.functions[1];
        let before = match boundary {
            0 => Instruction::HeapLoad {
                out: function.temps.alloc(Temp { ty: MANAGED_PTR }),
                object: Value::Temp(out),
                offset: 16,
            },
            1 => {
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
            _ => {
                let mut site = void_site(
                    &mut function.call_targets,
                    TestCallProtocol::Managed {
                        safepoint: 9,
                        destination: managed_runtime(scoop_lir::ManagedRuntimeFunction::Safepoint),
                    },
                    vec![],
                    vec![],
                );
                set_managed_live(
                    &mut site,
                    statepoint_live(vec![
                        statepoint_value(scoop_lir::CallerRootSource::Param(0), MANAGED_PTR, &[0]),
                        statepoint_value(scoop_lir::CallerRootSource::Temp(out), MANAGED_PTR, &[0]),
                    ]),
                );
                Instruction::Call { site }
            }
        };
        function.blocks[function.entry]
            .instructions
            .extend([before, store(Value::Temp(out))]);
        let ir = body(&mut module, scoop_lir::OptimizationMode::Release);
        assert_eq!(guards(&ir), 0, "{boundary}: {ir}");
        assert!(ir.contains("@scoop_gc_page_map"), "{ir}");
    }
}

#[test]
fn reference_initialization_crosses_unique_edges_but_not_joins() {
    for shared in [false, true] {
        let (mut module, out) = fixture(24, vec![16]);
        let function = &mut module.functions[1];
        let local = function.locals.alloc(test_local("target", MANAGED_PTR));
        function.blocks[function.entry]
            .instructions
            .push(Instruction::Store {
                local,
                value: Value::Temp(out),
            });
        let next = function.blocks.alloc(BasicBlock {
            name: "initialize".into(),
            instructions: vec![store(Value::Local(local))],
            terminator: Terminator::Return { value: None },
        });
        function.blocks[function.entry].terminator = Terminator::Br(next);
        if shared {
            function.signature =
                plain_scoop_signature(vec![MANAGED_PTR, LirType::I1], LirType::Void);
            let existing = function.blocks.alloc(BasicBlock {
                name: "existing".into(),
                instructions: vec![Instruction::Store {
                    local,
                    value: Value::Param(0),
                }],
                terminator: Terminator::Br(next),
            });
            function.entry = function.blocks.alloc(BasicBlock {
                name: "choose".into(),
                instructions: vec![],
                terminator: Terminator::CondBr {
                    cond: Value::Param(1),
                    then_block: function.entry,
                    else_block: existing,
                },
            });
        }
        let ir = body(&mut module, scoop_lir::OptimizationMode::Release);
        assert_eq!(guards(&ir), usize::from(!shared), "{ir}");
        assert!(ir.contains("@scoop_gc_page_map"), "{ir}");
    }
}

#[test]
fn aggregate_initialization_guards_the_complete_reference_range() {
    let (mut module, out) = fixture(32, vec![16, 24]);
    let function = &mut module.functions[1];
    let aggregate = function.temps.alloc(Temp {
        ty: LirType::Aggregate(vec![MANAGED_PTR, MANAGED_PTR]),
    });
    function.blocks[function.entry].instructions.extend([
        Instruction::MakeAggregate {
            out: aggregate,
            elements: vec![Value::Param(0), Value::Param(0)],
        },
        Instruction::HeapStore {
            object: Value::Temp(out),
            offset: 16,
            value: Value::Temp(aggregate),
        },
    ]);
    let ir = body(&mut module, scoop_lir::OptimizationMode::Release);
    assert_eq!(guards(&ir), 1, "{ir}");
    let (before, barrier) = ir.split_once("init.barrier:").unwrap();
    assert!(!before.contains("@scoop_rt_gc_write_barrier"), "{ir}");
    assert!(
        barrier
            .lines()
            .any(|line| line.contains("@scoop_rt_gc_write_barrier") && line.contains("i64 16")),
        "{ir}"
    );
}

#[test]
fn statically_slow_initialization_keeps_the_existing_region_reuse() {
    let (mut module, out) = fixture(32768, vec![16, 24]);
    let function = &mut module.functions[1];
    function.blocks[function.entry].instructions.extend([
        store(Value::Temp(out)),
        Instruction::HeapStore {
            object: Value::Temp(out),
            offset: 24,
            value: Value::Param(0),
        },
    ]);
    let ir = body(&mut module, scoop_lir::OptimizationMode::Release);
    assert_eq!(guards(&ir), 0, "{ir}");
    assert_eq!(ir.matches("@scoop_gc_page_map").count(), 1, "{ir}");
}
