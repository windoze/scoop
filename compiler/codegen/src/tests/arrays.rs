use super::*;

/// An M5-shaped module: ArrayAlloc with i64 and aggregate (Point)
/// elements, ArrayLen, bounds-checked ArrayGet / ArraySet, and
/// ArrayClone on both element shapes.
fn arrays_module() -> Module {
    let point = LirType::Aggregate(vec![LirType::I64, LirType::I64]);
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
    let mutable_int_array = array_type(
        &mut meta,
        "MutableArray<Int>",
        scoop_lir::ArrayKind::Mutable,
        LirType::I64,
        8,
        8,
        RefScan::None,
    );
    let point_array = array_type(
        &mut meta,
        "Array<Point>",
        scoop_lir::ArrayKind::Immutable,
        point.clone(),
        16,
        8,
        RefScan::None,
    );
    let mutable_point_array = array_type(
        &mut meta,
        "MutableArray<Point>",
        scoop_lir::ArrayKind::Mutable,
        point.clone(),
        16,
        8,
        RefScan::None,
    );

    let mut locals = Arena::default();
    let numbers = locals.alloc(Local {
        name: "numbers".to_string(),
        ty: MANAGED_PTR,
    });

    let mut temps = Arena::default();
    let t0 = temps.alloc(Temp { ty: MANAGED_PTR }); // array_alloc (1, 2, 3)
    let t1 = temps.alloc(Temp { ty: LirType::I64 }); // array_len t0
    let t2 = temps.alloc(Temp { ty: LirType::I64 }); // array_get t0[1]
    let t3 = temps.alloc(Temp { ty: MANAGED_PTR }); // array_clone t0
    let t4 = temps.alloc(Temp { ty: point.clone() }); // aggregate (t2, t1)
    let t5 = temps.alloc(Temp { ty: MANAGED_PTR }); // array_alloc (t4, t4)
    let t6 = temps.alloc(Temp { ty: point.clone() }); // array_get t5[1]
    let t7 = temps.alloc(Temp { ty: LirType::I64 }); // extract t6.1
    let t8 = temps.alloc(Temp { ty: MANAGED_PTR }); // array_clone t5
    let t9 = temps.alloc(Temp { ty: LirType::I64 }); // t1 + t7
    let t10 = temps.alloc(Temp { ty: MANAGED_PTR }); // [9, *numbers]
    let t11 = temps.alloc(Temp { ty: LirType::I64 }); // array_len t10

    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::ArrayAlloc {
                out: t0,
                elements: vec![signed64(1), signed64(2), signed64(3)],
                array_type: int_array,
                safepoint: test_safepoint(1),
                live: scoop_lir::StatepointLiveSet::default(),
            },
            Instruction::Store {
                local: numbers,
                value: Value::Temp(t0),
            },
            Instruction::ArrayLen {
                out: t1,
                operand: Value::Local(numbers),
                array_type: int_array,
            },
            Instruction::ArrayGet {
                out: t2,
                array: Value::Local(numbers),
                index: signed64(1),
                array_type: int_array,
            },
            Instruction::ArraySet {
                array: Value::Local(numbers),
                index: signed64(0),
                value: Value::Temp(t2),
                array_type: int_array,
            },
            Instruction::ArrayClone {
                out: t3,
                operand: Value::Local(numbers),
                array_type: mutable_int_array,
                safepoint: test_safepoint(2),
                live: statepoint_live(vec![statepoint_value(
                    scoop_lir::CallerRootSource::Local(numbers),
                    MANAGED_PTR,
                    &[0],
                )]),
            },
            Instruction::MakeAggregate {
                out: t4,
                elements: vec![Value::Temp(t2), Value::Temp(t1)],
            },
            Instruction::ArrayAlloc {
                out: t5,
                elements: vec![Value::Temp(t4), Value::Temp(t4)],
                array_type: point_array,
                safepoint: test_safepoint(3),
                live: statepoint_live(vec![statepoint_value(
                    scoop_lir::CallerRootSource::Temp(t3),
                    MANAGED_PTR,
                    &[0],
                )]),
            },
            Instruction::ArrayGet {
                out: t6,
                array: Value::Temp(t5),
                index: signed64(0),
                array_type: point_array,
            },
            Instruction::ExtractValue {
                out: t7,
                aggregate: Value::Temp(t6),
                index: 1,
            },
            Instruction::ArraySet {
                array: Value::Temp(t5),
                index: Value::Temp(t1),
                value: Value::Temp(t6),
                array_type: point_array,
            },
            Instruction::ArrayClone {
                out: t8,
                operand: Value::Temp(t5),
                array_type: mutable_point_array,
                safepoint: test_safepoint(4),
                live: statepoint_live(vec![
                    statepoint_value(scoop_lir::CallerRootSource::Temp(t3), MANAGED_PTR, &[0]),
                    statepoint_value(scoop_lir::CallerRootSource::Temp(t5), MANAGED_PTR, &[0]),
                ]),
            },
            Instruction::IntegerBinary {
                out: t9,
                kind: IntegerKind::SIGNED_64,
                operation: IntegerBinaryOperation::Add,
                lhs: Value::Temp(t1),
                rhs: Value::Temp(t7),
            },
            Instruction::ArraySet {
                array: Value::Temp(t3),
                index: signed64(0),
                value: signed64(0),
                array_type: mutable_int_array,
            },
            Instruction::ArraySet {
                array: Value::Temp(t8),
                index: signed64(0),
                value: Value::Temp(t6),
                array_type: mutable_point_array,
            },
            Instruction::ArrayAssembly {
                out: t10,
                parts: vec![
                    scoop_lir::ArrayAssemblyPart::Element(signed64(9)),
                    scoop_lir::ArrayAssemblyPart::CopyArray(Value::Local(numbers)),
                ],
                array_type: int_array,
                safepoint: test_safepoint(5),
                live: statepoint_live(vec![statepoint_value(
                    scoop_lir::CallerRootSource::Local(numbers),
                    MANAGED_PTR,
                    &[0],
                )]),
            },
            Instruction::ArrayLen {
                out: t11,
                operand: Value::Temp(t10),
                array_type: int_array,
            },
        ],
        terminator: Terminator::Return { value: None },
    });

    Module {
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
            callable_body: callable_body("scoop_main"),
            safepoints: test_safepoints("scoop_main", &blocks, entry),
            gc_effect: GcEffect::Managed,
            signature: plain_scoop_signature(vec![], LirType::Void),
            call_targets: CallTargets::default(),
            locals,
            temps,
            blocks,
            entry,
        }],
        output: scoop_lir::LirOutput::Executable {
            entry: managed_function_ref(0),
        },
        meta,
    }
}

#[test]
fn emits_m5_arrays() {
    let module = arrays_module();
    let int_array_symbol = type_descriptor_symbol(&module, "MutableArray<Int>");
    let point_array_symbol = type_descriptor_symbol(&module, "MutableArray<Point>");
    let ir = ir_of(&module);
    assert!(
        ir.lines().any(|line| {
            line.contains("call ptr addrspace(1) @scoop_rt_array_clone")
                && line.contains(&int_array_symbol)
        }),
        "Int clone must receive the target nominal descriptor:\n{ir}"
    );
    assert!(
        ir.contains("assembly_total_overflow")
            && ir.contains("assembly.copy.cond.1")
            && ir.contains("array size overflow"),
        "ArrayAssembly must check its dynamic size and copy spread elements:\n{ir}"
    );
    assert!(
        ir.lines().any(|line| {
            line.contains("call ptr addrspace(1) @scoop_rt_array_clone")
                && line.contains(&point_array_symbol)
        }),
        "Point clone must receive the target nominal descriptor:\n{ir}"
    );
    let output =
        std::env::temp_dir().join(format!("scoop_codegen_m5_test_{}.o", std::process::id()));
    write_verified_test_object(&module, &output);
    let len = std::fs::metadata(&output)
        .expect("object file exists")
        .len();
    assert!(len > 0, "object file is empty");
    std::fs::remove_file(&output).ok();
}

#[test]
fn array_assembly_checks_the_long_logical_length_limit() {
    let ir = ir_of(&arrays_module());
    assert!(
        ir.lines().any(|line| {
            line.contains("assembly_long_size_overflow = icmp ugt i64")
                && line.contains("9223372036854775807")
        }),
        "ArrayAssembly must reject logical lengths above Long.MAX_VALUE:\n{ir}"
    );
    assert!(
        ir.contains("assembly.size.long.ok.1"),
        "the Long length check must branch through the array-size trap:\n{ir}"
    );
}

fn array_codegen_error(module: &Module) -> CodegenError {
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    emit_llvm_module(&context, module, &machine, host_profile())
        .expect_err("malformed array LIR must be rejected")
}

#[test]
fn array_alloc_rejects_wrong_element_domain() {
    let mut module = arrays_module();
    let function = &mut module.functions[0];
    let Instruction::ArrayAlloc { elements, .. } =
        &mut function.blocks[function.entry].instructions[0]
    else {
        panic!("array fixture must start with ArrayAlloc")
    };
    elements[0] = Value::MachineScalar(MachineScalarValue::EnumTag(1));

    let error = array_codegen_error(&module);
    assert!(
        error.0.contains("array_alloc")
            && error.0.contains("machine<enum-tag>")
            && error.0.contains("expected i64"),
        "unexpected error: {error}"
    );
}

#[test]
fn array_get_rejects_machine_scalar_result_for_i64_element() {
    let mut module = arrays_module();
    let function = &mut module.functions[0];
    let out = match &function.blocks[function.entry].instructions[3] {
        Instruction::ArrayGet { out, .. } => *out,
        _ => panic!("array fixture must get its first element"),
    };
    function.temps[out].ty = LirType::MachineScalar(MachineScalarKind::EnumTag);

    let error = array_codegen_error(&module);
    assert!(
        error.0.contains("array_get")
            && error.0.contains("machine<enum-tag>")
            && error.0.contains("i64"),
        "unexpected error: {error}"
    );
}

#[test]
fn array_set_rejects_machine_scalar_for_i64_element() {
    let mut module = arrays_module();
    let function = &mut module.functions[0];
    let Instruction::ArraySet { value, .. } = &mut function.blocks[function.entry].instructions[4]
    else {
        panic!("array fixture must set its first element")
    };
    *value = Value::MachineScalar(MachineScalarValue::EnumTag(0));

    let error = array_codegen_error(&module);
    assert!(
        error.0.contains("array_set")
            && error.0.contains("machine<enum-tag>")
            && error.0.contains("i64"),
        "unexpected error: {error}"
    );
}

#[test]
fn array_assembly_rejects_wrong_element_domain() {
    let mut module = arrays_module();
    let function = &mut module.functions[0];
    let Instruction::ArrayAssembly { parts, .. } =
        &mut function.blocks[function.entry].instructions[15]
    else {
        panic!("array fixture must assemble its spread array")
    };
    parts[0] =
        scoop_lir::ArrayAssemblyPart::Element(Value::MachineScalar(MachineScalarValue::EnumTag(9)));

    let error = array_codegen_error(&module);
    assert!(
        error.0.contains("array_assembly")
            && error.0.contains("machine<enum-tag>")
            && error.0.contains("expected i64"),
        "unexpected error: {error}"
    );
}

#[test]
fn array_metadata_rejects_recursive_machine_scalar_element() {
    let mut module = arrays_module();
    let (_, array) = module
        .meta
        .arrays
        .iter_mut()
        .next()
        .expect("array metadata");
    array.element = LirType::Aggregate(vec![LirType::MachineScalar(MachineScalarKind::EnumTag)]);

    let error = array_codegen_error(&module);
    assert!(
        error.0.contains("array element type") && error.0.contains("machine scalar"),
        "unexpected error: {error}"
    );
}
