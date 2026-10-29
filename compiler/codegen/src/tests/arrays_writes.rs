use super::*;
use inkwell::values::AnyValue;
use scoop_lir::OptimizationMode;

fn fixture(element: LirType, size: u64, scan: RefScan) -> Module {
    let mut module = arrays::arrays_module();
    module.output = scoop_lir::LirOutput::Library;
    let array_type = array_type(
        &mut module.meta,
        "MutableArray<Entry>",
        scoop_lir::ArrayKind::Mutable,
        element.clone(),
        size,
        8,
        scan,
    );
    let function = &mut module.functions[0];
    function.signature =
        plain_scoop_signature(vec![MANAGED_PTR, LirType::I64, element], LirType::Void);
    function.call_targets = CallTargets::default();
    function.locals = Arena::default();
    function.temps = Arena::default();
    function.blocks = Arena::default();
    function.entry = function.blocks.alloc(BasicBlock {
        terminator_origin: Default::default(),
        name: "entry".into(),
        instructions: vec![Instruction::ArraySet {
            array: Value::Param(0),
            index: Value::Param(1),
            value: Value::Param(2),
            array_type,
        }]
        .into(),
        terminator: Terminator::Return { value: None },
    });
    refresh_module_safepoints(&mut module);
    module
}

fn wide_element() -> LirType {
    LirType::Aggregate(vec![LirType::I64, MANAGED_PTR, LirType::I64])
}

fn body(module: &Module, mode: OptimizationMode, optimize: bool) -> String {
    let context = Context::create();
    let machine = host_target_machine().unwrap();
    let profile = host_profile().with_optimization(mode);
    let llvm = emit_llvm_module(&context, module, &machine, profile).unwrap();
    if optimize {
        let expected = statepoint::expectations(module).unwrap();
        statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
    }
    llvm.verify().unwrap();
    llvm.get_function(module.functions[0].symbol())
        .unwrap()
        .print_to_string()
        .to_string()
}

#[test]
fn wide_array_writes_guard_the_actual_reference_slot_after_the_full_store() {
    let module = fixture(wide_element(), 24, RefScan::References(vec![8]));
    let ir = body(&module, OptimizationMode::Release, false);
    let old = ir
        .find("write.old_reference = load ptr addrspace(1)")
        .unwrap();
    let store = ir.find("store { i64, ptr addrspace(1), i64 }").unwrap();
    let new = ir
        .find("write.new_reference = load ptr addrspace(1)")
        .unwrap();
    let mark = ir
        .find("store atomic i8 1")
        .unwrap_or_else(|| panic!("missing card mark after the guarded store:\n{ir}"));
    assert!(old < store && store < new && new < mark, "{ir}");
    assert!(ir.contains("ptr addrspace(1) %element_ptr, i64 8"), "{ir}");
    assert!(
        ir.contains("icmp ne ptr addrspace(1) %write.old_reference, %write.new_reference"),
        "{ir}"
    );
    assert!(
        ir.contains("br i1 %write.needs_barrier, label %write.barrier, label %write.done"),
        "{ir}"
    );
    let optimized = body(&module, OptimizationMode::Release, true);
    assert!(
        !optimized.contains("write.new_reference = load"),
        "{optimized}"
    );
    assert!(optimized.contains("store atomic i8 1"), "{optimized}");
}

#[test]
fn guards_do_not_replace_debug_small_element_or_multiple_reference_barriers() {
    let wide = fixture(wide_element(), 24, RefScan::References(vec![8]));
    let debug = body(&wide, OptimizationMode::Debug, false);
    assert!(!debug.contains("write.old_reference"), "{debug}");
    assert!(debug.contains("store atomic i8 1"), "{debug}");
    for (element, size, refs) in [
        (MANAGED_PTR, 8, vec![0]),
        (LirType::Interface, 16, vec![0]),
        (
            LirType::Aggregate(vec![MANAGED_PTR, MANAGED_PTR, LirType::I64]),
            24,
            vec![0, 8],
        ),
    ] {
        let module = fixture(element, size, RefScan::References(refs));
        let ir = body(&module, OptimizationMode::Release, false);
        assert!(!ir.contains("write.old_reference"), "{ir}");
        assert!(
            ir.contains("store atomic i8 1") || ir.contains("call void @scoop_rt_gc_write_barrier"),
            "{ir}"
        );
    }
}

#[test]
fn unchanged_and_null_references_remove_marks_but_keep_scalar_updates() {
    for clear in [false, true] {
        let mut module = fixture(wide_element(), 24, RefScan::References(vec![8]));
        let function = &mut module.functions[0];
        let old = function.temps.alloc(Temp { ty: wide_element() });
        let reference = function.temps.alloc(Temp { ty: MANAGED_PTR });
        let new = function.temps.alloc(Temp { ty: wide_element() });
        let Instruction::ArraySet { array_type, .. } =
            function.blocks[function.entry].instructions[0]
        else {
            unreachable!()
        };
        function.blocks[function.entry].instructions = vec![
            Instruction::ArrayGet {
                out: old,
                array: Value::Param(0),
                index: Value::Param(1),
                array_type,
            },
            Instruction::ExtractValue {
                out: reference,
                aggregate: Value::Temp(old),
                index: 1,
            },
            Instruction::MakeAggregate {
                out: new,
                elements: vec![
                    signed64(17),
                    if clear {
                        Value::NullPointer(PointerKind::Managed)
                    } else {
                        Value::Temp(reference)
                    },
                    signed64(23),
                ],
            },
            Instruction::ArraySet {
                array: Value::Param(0),
                index: Value::Param(1),
                value: Value::Temp(new),
                array_type,
            },
        ]
        .into();
        let ir = body(&module, OptimizationMode::Release, true);
        assert!(!ir.contains("load atomic ptr"), "{ir}");
        assert!(!ir.contains("store atomic i8 1"), "{ir}");
        assert!(
            ir.contains("store i64 17") && ir.contains("store i64 23"),
            "{ir}"
        );
    }
}
