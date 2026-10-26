use super::*;

fn field(object: u32, offset: u64) -> Instruction {
    Instruction::HeapStore {
        object: Value::Param(object),
        offset,
        value: Value::Param(2),
    }
}

fn fixture(instructions: Vec<Instruction>) -> Module {
    let mut module = values_module();
    module.output = scoop_lir::LirOutput::Library;
    module.functions.truncate(1);
    let function = &mut module.functions[0];
    function.signature = plain_scoop_signature(vec![MANAGED_PTR; 3], LirType::Void);
    function.call_targets = CallTargets::default();
    function.locals = Arena::default();
    function.temps = Arena::default();
    function.blocks = Arena::default();
    function.entry = function.blocks.alloc(BasicBlock {
        name: "fields".into(),
        instructions,
        terminator: Terminator::Return { value: None },
    });
    refresh_module_safepoints(&mut module);
    module
}

fn assert_work(module: &Module, lookups: usize, marks: usize) {
    let ir = ir_of(module);
    assert_eq!(ir.matches("load atomic ptr").count(), lookups * 4, "{ir}");
    assert_eq!(ir.matches("store atomic i8 1, ptr ").count(), marks, "{ir}");
}

#[test]
fn field_stores_reuse_only_one_objects_mapping_and_exact_slot_marks() {
    assert_work(&fixture(vec![field(0, 16), field(0, 16)]), 1, 1);
    // These slots can occupy different cards; only the mapping is reused.
    assert_work(&fixture(vec![field(0, 16), field(0, 24)]), 1, 2);
    assert_work(&fixture(vec![field(0, 16), field(0, 528)]), 1, 2);
    assert_work(&fixture(vec![field(0, 16), field(1, 16)]), 2, 2);
}

#[test]
fn field_barriers_do_not_cross_atomic_publication_or_blocks() {
    let atomic = Instruction::AtomicStore {
        location: scoop_lir::AtomicLocation {
            object: Value::Param(1),
            offset: 16,
            kind: scoop_lir::AtomicValueKind::Reference,
        },
        value: Value::Param(2),
        order: scoop_lir::AtomicStoreOrder::Release,
    };
    assert_work(&fixture(vec![field(0, 16), atomic, field(0, 16)]), 3, 3);
    let mut module = fixture(vec![field(0, 16)]);
    let function = &mut module.functions[0];
    let next = function.blocks.alloc(BasicBlock {
        name: "next".into(),
        instructions: vec![field(0, 16)],
        terminator: Terminator::Return { value: None },
    });
    function.blocks[function.entry].terminator = Terminator::Br(next);
    assert_work(&module, 2, 2);
}

#[test]
fn field_barriers_are_rebuilt_after_a_collecting_call() {
    let mut module = fixture(vec![field(0, 16)]);
    let function = &mut module.functions[0];
    let mut site = void_site(
        &mut function.call_targets,
        TestCallProtocol::Managed {
            safepoint: 1,
            destination: managed_runtime(scoop_lir::ManagedRuntimeFunction::Safepoint),
        },
        vec![],
        vec![],
    );
    set_managed_live(
        &mut site,
        statepoint_live(
            [0, 2]
                .into_iter()
                .map(|index| {
                    statepoint_value(scoop_lir::CallerRootSource::Param(index), MANAGED_PTR, &[0])
                })
                .collect(),
        ),
    );
    function.blocks[function.entry]
        .instructions
        .extend([Instruction::Call { site }, field(0, 16)]);
    refresh_module_safepoints(&mut module);
    assert_work(&module, 2, 2);
}

#[test]
fn no_gc_calls_also_end_the_local_field_write_segment() {
    let mut module = fixture(vec![field(0, 16)]);
    let function = &mut module.functions[0];
    let out = function.temps.alloc(Temp { ty: LirType::I64 });
    let site = direct_site(
        &mut function.call_targets,
        TestCallProtocol::NoGc {
            destination: no_gc_runtime(scoop_lir::NoGcRuntimeFunction::GcStats),
        },
        vec![],
        (LirType::I64, RefScan::None),
        out,
        vec![],
    );
    function.blocks[function.entry]
        .instructions
        .extend([Instruction::Call { site }, field(0, 16)]);
    assert_work(&module, 2, 2);
}

#[test]
fn changing_a_local_receiver_discards_its_cached_region() {
    let mut module = fixture(vec![]);
    let function = &mut module.functions[0];
    let local = function.locals.alloc(Local::new(
        "receiver",
        scoop_lir::LocalStorage::NonZero(abi_value_with_layout(
            MANAGED_PTR,
            8,
            8,
            RefScan::References(vec![0]),
        )),
    ));
    for parameter in [0, 1] {
        function.blocks[function.entry].instructions.extend([
            Instruction::Store {
                local,
                value: Value::Param(parameter),
            },
            Instruction::HeapStore {
                object: Value::Local(local),
                offset: 16,
                value: Value::Param(2),
            },
        ]);
    }
    assert_work(&module, 2, 2);
}
