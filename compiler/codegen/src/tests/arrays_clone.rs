use super::*;

pub(super) fn reference_clone_module() -> Module {
    let mut module = arrays::arrays_module();
    module.output = scoop_lir::LirOutput::Library;
    let element = LirType::Aggregate(vec![MANAGED_PTR, LirType::I64]);
    let source = array_type(
        &mut module.meta,
        "Array<Reference>",
        scoop_lir::ArrayKind::Immutable,
        element.clone(),
        16,
        8,
        RefScan::References(vec![0]),
    );
    let target = array_type(
        &mut module.meta,
        "MutableArray<Reference>",
        scoop_lir::ArrayKind::Mutable,
        element,
        16,
        8,
        RefScan::References(vec![0]),
    );
    let mut temps = Arena::default();
    let copy = temps.alloc(Temp { ty: MANAGED_PTR });
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".into(),
        instructions: vec![Instruction::ArrayClone {
            out: copy,
            operand: Value::Param(0),
            source_type: source,
            array_type: target,
            safepoint: test_safepoint(1),
            live: statepoint_live(vec![statepoint_value(
                scoop_lir::CallerRootSource::Param(0),
                MANAGED_PTR,
                &[0],
            )]),
        }],
        terminator: Terminator::Return {
            value: Some(Value::Temp(copy)),
        },
    });
    module.functions[0] = Function {
        callable_body: callable_body("scoop_main"),
        safepoints: test_safepoints("scoop_main", &blocks, entry),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(vec![MANAGED_PTR], MANAGED_PTR),
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps,
        blocks,
        entry,
    };
    module
}

#[test]
fn clone_reloads_its_only_source_root_and_barriers_reference_storage() {
    let module = reference_clone_module();
    let ir = ir_of(&module);
    let after_allocation = ir.split("alloc.continue.0:").nth(1).unwrap();
    let root = after_allocation
        .find("load ptr addrspace(1), ptr %managed_root_storage")
        .unwrap();
    let source = after_allocation
        .find("clone.source = getelementptr i8, ptr addrspace(1) %root_param")
        .unwrap();
    let copy = after_allocation.find("@llvm.memcpy.p1.p1.i64").unwrap();
    let barrier = after_allocation.find("@scoop_rt_gc_write_barrier").unwrap();
    assert!(root < source && source < copy && copy < barrier, "{ir}");
    assert!(!ir.contains("@scoop_rt_array_clone"), "{ir}");
    let rewritten = rewritten_ir_of(&module);
    assert_eq!(
        rewritten
            .matches("call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint")
            .count(),
        1,
        "{rewritten}"
    );
    assert!(rewritten.contains("gc.relocate"), "{rewritten}");
    assert!(rewritten.contains("@llvm.memcpy.p1.p1.i64"), "{rewritten}");
}
