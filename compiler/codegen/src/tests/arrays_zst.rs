use super::*;

#[test]
fn zst_arrays_keep_count_checks_without_payload_access() {
    let mut module = arrays::arrays_module();
    let overflow_message = module
        .globals
        .iter()
        .find_map(|(id, global)| matches!(global.init, GlobalInit::CString { .. }).then_some(id))
        .expect("the array callable owns its overflow message");
    let zst = LirType::Aggregate(vec![]);
    let source = array_type(
        &mut module.meta,
        "MutableArray<Empty>",
        scoop_lir::ArrayKind::Mutable,
        zst.clone(),
        0,
        1,
        RefScan::None,
    );
    let target = array_type(
        &mut module.meta,
        "Array<Empty>",
        scoop_lir::ArrayKind::Immutable,
        zst.clone(),
        0,
        1,
        RefScan::None,
    );
    let source_symbol = type_descriptor_symbol(&module, "MutableArray<Empty>");
    let target_symbol = type_descriptor_symbol(&module, "Array<Empty>");
    let mut temps = Arena::default();
    let value = temps.alloc(Temp { ty: zst.clone() });
    let literal = temps.alloc(Temp { ty: MANAGED_PTR });
    let assembled = temps.alloc(Temp { ty: MANAGED_PTR });
    let loaded = temps.alloc(Temp { ty: zst });
    let cloned = temps.alloc(Temp { ty: MANAGED_PTR });
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".into(),
        instructions: vec![
            Instruction::MakeAggregate {
                out: value,
                elements: vec![],
            },
            Instruction::ArrayAlloc {
                out: literal,
                elements: vec![Value::Temp(value); 3],
                array_type: source,
                safepoint: test_safepoint(1),
                live: Default::default(),
            },
            Instruction::ArrayAssembly {
                out: assembled,
                overflow_message,
                parts: vec![
                    scoop_lir::ArrayAssemblyPart::CopyArray(Value::Temp(literal)),
                    scoop_lir::ArrayAssemblyPart::Element(Value::Temp(value)),
                ],
                array_type: source,
                safepoint: test_safepoint(2),
                live: statepoint_live(vec![statepoint_value(
                    scoop_lir::CallerRootSource::Temp(literal),
                    MANAGED_PTR,
                    &[0],
                )]),
            },
            Instruction::ArrayGet {
                out: loaded,
                array: Value::Temp(assembled),
                index: signed64(0),
                array_type: source,
            },
            Instruction::ArraySet {
                array: Value::Temp(assembled),
                index: signed64(1),
                value: Value::Temp(loaded),
                array_type: source,
            },
            Instruction::ArrayClone {
                out: cloned,
                operand: Value::Temp(assembled),
                source_type: source,
                array_type: target,
                safepoint: test_safepoint(3),
                live: statepoint_live(vec![statepoint_value(
                    scoop_lir::CallerRootSource::Temp(assembled),
                    MANAGED_PTR,
                    &[0],
                )]),
            },
        ],
        terminator: Terminator::Return { value: None },
    });
    module.functions[0] = Function {
        callable_body: callable_body("scoop_main"),
        safepoints: test_safepoints("scoop_main", &blocks, entry),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(vec![], LirType::Void),
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps,
        blocks,
        entry,
    };
    let ir = ir_of(&module);
    assert!(
        ir.contains("assembly_long_size_overflow"),
        "ZST retains logical count checks:\n{ir}"
    );
    for forbidden in [
        "out_of_bounds",
        "element_ptr",
        "assembly.copy.cond",
        "assembly_element_bytes",
        "card_index",
        "load {}",
        "store {}",
    ] {
        assert!(
            !ir.contains(forbidden),
            "ZST emitted payload operation {forbidden}:\n{ir}"
        );
    }
    assert!(
        ir.lines().any(
            |line| line.contains("call ptr addrspace(1) @scoop_rt_array_clone")
                && line.contains(&source_symbol)
                && line.contains(&target_symbol)
        ),
        "clone retains both exact descriptors:\n{ir}"
    );
    assert!(
        ir.contains("allocation_alignment") && ir.contains("tlab_aligned_cursor"),
        "TLAB allocation consumes descriptor alignment:\n{ir}"
    );
}

#[test]
fn array_clone_rejects_different_exact_elements_with_equal_storage() {
    let mut module = arrays::arrays_module();
    let (_, target) = module
        .meta
        .arrays
        .iter_mut()
        .find(|(_, array)| {
            array.kind == scoop_lir::ArrayKind::Mutable && array.element == LirType::I64
        })
        .unwrap();
    target.element_exact = test_exact_type("DifferentExactInteger");
    let error = validation::validate_module(&module)
        .expect_err("equal physical layouts cannot replace exact array element identity");
    assert!(
        error.0.contains("array clone exact element type"),
        "{error}"
    );
}
