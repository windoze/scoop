use super::*;
use inkwell::values::AnyValue;

fn reference_arrays() -> Vec<Module> {
    let clone = arrays_clone::reference_clone_module();
    let mut literal = arrays_clone::reference_clone_module();
    let function = &mut literal.functions[0];
    let Instruction::ArrayClone {
        out,
        array_type,
        safepoint,
        ..
    } = function.blocks[function.entry].instructions[0]
    else {
        unreachable!()
    };
    let element = LirType::Aggregate(vec![MANAGED_PTR, LirType::I64]);
    function.signature = plain_scoop_signature(vec![element.clone()], MANAGED_PTR);
    function.blocks[function.entry].instructions = vec![Instruction::ArrayAlloc {
        out,
        array_type,
        safepoint,
        elements: vec![Value::Param(0); 3],
        live: statepoint_live(vec![statepoint_value(
            scoop_lir::CallerRootSource::Param(0),
            element.clone(),
            &[0],
        )]),
    }];
    let mut assembly = arrays_clone::reference_clone_module();
    let overflow_message = assembly
        .globals
        .iter()
        .find_map(|(id, global)| matches!(global.init, GlobalInit::CString { .. }).then_some(id))
        .unwrap();
    let function = &mut assembly.functions[0];
    let Instruction::ArrayClone {
        out,
        safepoint,
        source_type,
        ..
    } = function.blocks[function.entry].instructions[0]
    else {
        unreachable!()
    };
    function.signature = plain_scoop_signature(vec![MANAGED_PTR, element.clone()], MANAGED_PTR);
    function.blocks[function.entry].instructions = vec![Instruction::ArrayAssembly {
        out,
        array_type: source_type,
        safepoint,
        overflow_message,
        parts: vec![
            scoop_lir::ArrayAssemblyPart::Element(Value::Param(1)),
            scoop_lir::ArrayAssemblyPart::CopyArray(Value::Param(0)),
            scoop_lir::ArrayAssemblyPart::Element(Value::Param(1)),
        ],
        live: statepoint_live(vec![
            statepoint_value(scoop_lir::CallerRootSource::Param(0), MANAGED_PTR, &[0]),
            statepoint_value(scoop_lir::CallerRootSource::Param(1), element, &[0]),
        ]),
    }];
    vec![clone, literal, assembly]
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
    llvm.get_function(module.functions[0].symbol())
        .unwrap()
        .print_to_string()
        .to_string()
}

#[test]
fn reference_arrays_skip_the_range_barrier_only_on_the_actual_tlab_branch() {
    for mut module in reference_arrays() {
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
        let (initialization, barrier) = ir.split_once("init.barrier:").unwrap();
        assert!(
            !initialization.contains("@scoop_rt_gc_write_barrier"),
            "{ir}"
        );
        assert!(initialization.contains("@scoop_runtime_alloc_slow"), "{ir}");
        assert!(
            initialization.contains("store { ptr addrspace(1), i64 }")
                || initialization.contains("@llvm.memcpy.p1.p1.i64"),
            "{ir}"
        );
        assert!(barrier.contains("@scoop_rt_gc_write_barrier"), "{ir}");
        assert!(barrier.contains("barrier_elements"), "{ir}");
        assert!(barrier.contains("i64 24"), "{ir}");

        let debug = body(&mut module, scoop_lir::OptimizationMode::Debug);
        assert!(!debug.contains("init.barrier"), "{debug}");
        assert!(debug.contains("@scoop_rt_gc_write_barrier"), "{debug}");
    }
}

#[test]
fn scalar_and_empty_arrays_do_not_gain_initialization_guards() {
    let ir = body(
        &mut arrays::arrays_module(),
        scoop_lir::OptimizationMode::Release,
    );
    assert!(!ir.contains("init.barrier"), "{ir}");
    assert!(!ir.contains("@scoop_rt_gc_write_barrier"), "{ir}");

    let mut module = reference_arrays().remove(1);
    let function = &mut module.functions[0];
    let Instruction::ArrayAlloc { elements, .. } =
        &mut function.blocks[function.entry].instructions[0]
    else {
        unreachable!()
    };
    elements.clear();
    let ir = body(&mut module, scoop_lir::OptimizationMode::Release);
    assert!(!ir.contains("init.barrier"), "{ir}");
    assert!(!ir.contains("@scoop_rt_gc_write_barrier"), "{ir}");
}
