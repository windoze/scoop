use super::*;
use scoop_lir::LirIntegerConstant;

#[path = "integers.rs"]
mod integers;

#[test]
fn emits_non_empty_object_file() {
    let module = values_module();
    let output = std::env::temp_dir().join(format!("scoop_codegen_test_{}.o", std::process::id()));
    write_verified_test_object(&module, &output);
    let len = std::fs::metadata(&output)
        .expect("object file exists")
        .len();
    assert!(len > 0, "object file is empty");
    std::fs::remove_file(&output).ok();
}

#[test]
fn strong_codegen_rejects_odr_callable_bodies() {
    let mut module = values_module();
    module.functions[0].callable_body = odr_callable_body("shared_function");
    refresh_test_safepoints(&mut module.functions[0]);
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("the M23-3 backend accepts strong callable bodies only");
    assert!(
        error.0.contains("ODR") || error.0.contains("Odr"),
        "{error}"
    );
}

#[test]
fn executable_emits_only_the_typed_persistent_entry_body() {
    let module = values_module();
    let entry = &module.functions[module
        .executable_entry()
        .expect("test module is executable")
        .declaration()
        .into_u32() as usize];

    let ir = ir_of(&module);

    assert!(!ir.contains("define void @scoop_main()"), "{ir}");
    assert!(
        ir.contains(&format!("define void @\"{}\"()", entry.symbol())),
        "{ir}"
    );
}

#[test]
fn library_does_not_emit_a_fixed_entry_symbol() {
    let mut module = values_module();
    module.output = scoop_lir::LirOutput::Library;

    let ir = ir_of(&module);

    assert!(!ir.contains("@scoop_main"), "{ir}");
}

#[test]
fn emits_unsigned_division_remainder_and_three_way_comparisons() {
    let mut module = values_module();
    let function = &mut module.functions[0];
    let entry = function.entry;
    let input = function.locals.iter().next().expect("integer local").0;
    let quotient = function.temps.alloc(Temp { ty: LirType::I64 });
    let remainder = function.temps.alloc(Temp { ty: LirType::I64 });
    let signed_compare = function.temps.alloc(Temp { ty: LirType::I64 });
    let unsigned_compare = function.temps.alloc(Temp { ty: LirType::I64 });
    function.blocks[entry].instructions.extend([
        Instruction::SafeIntegerDivRem {
            out: quotient,
            kind: IntegerKind::UNSIGNED_64,
            operation: IntegerDivRemOperation::Divide,
            lhs: Value::Local(input),
            rhs: Value::IntegerConst(LirIntegerConstant::Unsigned64(2)),
        },
        Instruction::SafeIntegerDivRem {
            out: remainder,
            kind: IntegerKind::UNSIGNED_64,
            operation: IntegerDivRemOperation::Remainder,
            lhs: Value::Local(input),
            rhs: Value::IntegerConst(LirIntegerConstant::Unsigned64(2)),
        },
        Instruction::IntegerCompareTo {
            out: signed_compare,
            operand_kind: IntegerKind::SIGNED_64,
            lhs: Value::Local(input),
            rhs: signed64(2),
        },
        Instruction::IntegerCompareTo {
            out: unsigned_compare,
            operand_kind: IntegerKind::UNSIGNED_64,
            lhs: Value::Local(input),
            rhs: Value::IntegerConst(LirIntegerConstant::Unsigned64(2)),
        },
    ]);
    let ir = ir_of(&module);
    for instruction in ["udiv i64", "urem i64", "icmp slt i64", "icmp ult i64"] {
        assert!(
            ir.contains(instruction),
            "missing {instruction:?} in:\n{ir}"
        );
    }
    assert!(ir.matches("select i1").count() >= 4, "{ir}");
}

#[test]
fn source_operators_reject_machine_scalar_operands() {
    let mut module = values_module();
    let function = &mut module.functions[0];
    let out = function.temps.alloc(Temp { ty: LirType::I1 });
    function.blocks[function.entry]
        .instructions
        .push(Instruction::BinOp {
            out,
            op: BinOp::Eq,
            lhs: Value::MachineScalar(MachineScalarValue::EnumTag(0)),
            rhs: Value::MachineScalar(MachineScalarValue::EnumTag(0)),
        });

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("source equality must not consume internal machine scalars");
    assert!(
        error.0.contains("generic equality Eq") && error.0.contains("machine<enum-tag>"),
        "unexpected error: {error}"
    );
}

#[test]
fn machine_equality_rejects_cross_domain_operands() {
    let mut module = values_module();
    let function = &mut module.functions[0];
    let out = function.temps.alloc(Temp { ty: LirType::I1 });
    function.blocks[function.entry]
        .instructions
        .push(Instruction::BinOp {
            out,
            op: BinOp::MachineEq(MachineScalarKind::EnumTag),
            lhs: Value::MachineScalar(MachineScalarValue::EnumTag(0)),
            rhs: Value::MachineScalar(MachineScalarValue::InitializationOutcome(
                scoop_lir::InitializationOutcome::Ready,
            )),
        });

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("machine equality must keep both operands in one domain");
    assert!(
        error
            .0
            .contains("non-source-integer binary MachineEq(EnumTag)")
            && error.0.contains("machine<enum-tag>")
            && error.0.contains("machine<initialization-outcome>"),
        "unexpected error: {error}"
    );
}

#[test]
fn conditional_branch_rejects_machine_scalar_constant() {
    let mut module = values_module();
    let function = &mut module.functions[0];
    let Terminator::CondBr { cond, .. } = &mut function.blocks[function.entry].terminator else {
        panic!("values fixture entry must end in a conditional branch")
    };
    *cond = Value::MachineScalar(MachineScalarValue::EnumTag(0));

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("a conditional branch must reject a machine-scalar constant");
    assert!(
        error.0.contains("cbr @entry")
            && error.0.contains("machine<enum-tag>")
            && error.0.contains("expected i1"),
        "unexpected error: {error}"
    );
}

#[test]
fn conditional_branch_rejects_machine_scalar_temp_before_materialization() {
    let mut module = values_module();
    let function = &mut module.functions[0];
    let machine_temp = function.temps.alloc(Temp {
        ty: LirType::MachineScalar(MachineScalarKind::EnumTag),
    });
    let Terminator::CondBr { cond, .. } = &mut function.blocks[function.entry].terminator else {
        panic!("values fixture entry must end in a conditional branch")
    };
    *cond = Value::Temp(machine_temp);

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("a conditional branch must reject a machine temp before materializing it");
    assert!(
        error.0.contains("cbr @entry")
            && error.0.contains("machine<enum-tag>")
            && error.0.contains("expected i1"),
        "unexpected error: {error}"
    );
}

#[test]
fn integer_to_pointer_rejects_machine_scalar_input() {
    let mut module = values_module();
    let function = &mut module.functions[0];
    let out = function.temps.alloc(Temp { ty: RAW_PTR });
    function.blocks[function.entry]
        .instructions
        .push(Instruction::ULongToPtr {
            out,
            value: Value::MachineScalar(MachineScalarValue::PointerElementOffset(1)),
        });

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("a machine scalar must not acquire source pointer-conversion semantics");
    assert!(
        error.0.contains("requires ULong/i64 -> ptr<raw>")
            && error.0.contains("machine<pointer-element-offset>"),
        "unexpected error: {error}"
    );
}

#[test]
fn pointer_to_integer_rejects_machine_scalar_output() {
    let mut module = values_module();
    let function = &mut module.functions[0];
    let out = function.temps.alloc(Temp {
        ty: LirType::MachineScalar(MachineScalarKind::EnumTag),
    });
    function.blocks[function.entry]
        .instructions
        .push(Instruction::PtrToULong {
            out,
            value: Value::NullPointer(PointerKind::Raw),
        });

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("a pointer conversion must not produce a machine scalar");
    assert!(
        error.0.contains("requires ptr<raw> -> ULong/i64") && error.0.contains("machine<enum-tag>"),
        "unexpected error: {error}"
    );
}

#[test]
fn aggregate_construction_rejects_machine_scalar_as_i64() {
    let mut module = values_module();
    let function = &mut module.functions[0];
    let Instruction::MakeAggregate { elements, .. } =
        &mut function.blocks[function.entry].instructions[2]
    else {
        panic!("values fixture must construct its point")
    };
    elements[0] = Value::MachineScalar(MachineScalarValue::EnumTag(40));

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("aggregate fields must have their exact logical LIR type");
    assert!(
        error.0.contains("aggregate construction")
            && error.0.contains("machine<enum-tag>")
            && error.0.contains("expected i64"),
        "unexpected error: {error}"
    );
}

#[test]
fn aggregate_extraction_rejects_i64_as_machine_scalar() {
    let mut module = values_module();
    let function = &mut module.functions[0];
    let out = match &function.blocks[function.entry].instructions[4] {
        Instruction::ExtractValue { out, .. } => *out,
        _ => panic!("values fixture must extract its point field"),
    };
    function.temps[out].ty = LirType::MachineScalar(MachineScalarKind::EnumTag);

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("aggregate extraction must preserve its logical field type");
    assert!(
        error.0.contains("aggregate extraction")
            && error.0.contains("machine<enum-tag>")
            && error.0.contains("expected i64"),
        "unexpected error: {error}"
    );
}

#[test]
fn raw_load_rejects_machine_scalar_result() {
    let mut module = values_module();
    let function = &mut module.functions[0];
    let local = function.locals.iter().next().expect("integer local").0;
    let pointer = function.temps.alloc(Temp { ty: RAW_PTR });
    let out = function.temps.alloc(Temp {
        ty: LirType::MachineScalar(MachineScalarKind::EnumTag),
    });
    function.blocks[function.entry].instructions.extend([
        Instruction::LocalAddress {
            out: pointer,
            local,
        },
        Instruction::RawLoad {
            out,
            pointer: Value::Temp(pointer),
            align: 8,
        },
    ]);

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("raw memory must not manufacture an internal machine scalar");
    assert!(
        error.0.contains("raw_load") && error.0.contains("machine<enum-tag>"),
        "unexpected error: {error}"
    );
}

#[test]
fn raw_store_rejects_machine_scalar_value() {
    let mut module = values_module();
    let function = &mut module.functions[0];
    let local = function.locals.iter().next().expect("integer local").0;
    let pointer = function.temps.alloc(Temp { ty: RAW_PTR });
    function.blocks[function.entry].instructions.extend([
        Instruction::LocalAddress {
            out: pointer,
            local,
        },
        Instruction::RawStore {
            pointer: Value::Temp(pointer),
            value: Value::MachineScalar(MachineScalarValue::EnumTag(0)),
            align: 8,
        },
    ]);

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("raw memory must not consume an internal machine scalar");
    assert!(
        error.0.contains("raw_store") && error.0.contains("machine<enum-tag>"),
        "unexpected error: {error}"
    );
}

#[test]
fn local_address_rejects_machine_scalar_storage() {
    let mut module = values_module();
    let function = &mut module.functions[0];
    let local = function.locals.alloc(Local {
        name: "machine_state".to_string(),
        ty: LirType::MachineScalar(MachineScalarKind::CoroutineFrameState),
    });
    let out = function.temps.alloc(Temp { ty: RAW_PTR });
    function.blocks[function.entry]
        .instructions
        .push(Instruction::LocalAddress { out, local });

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("a machine scalar local must not become an untyped raw payload");
    assert!(
        error.0.contains("local_address") && error.0.contains("machine<coroutine-frame-state>"),
        "unexpected error: {error}"
    );
}

#[test]
fn pointer_element_offset_scales_by_the_declared_element_size() {
    let mut module = values_module();
    let function = &mut module.functions[0];
    let local = function.locals.iter().next().expect("integer local").0;
    let pointer = function.temps.alloc(Temp { ty: RAW_PTR });
    let out = function.temps.alloc(Temp { ty: RAW_PTR });
    function.blocks[function.entry].instructions.extend([
        Instruction::LocalAddress {
            out: pointer,
            local,
        },
        Instruction::PtrOffset {
            out,
            pointer: Value::Temp(pointer),
            element_offset: Value::MachineScalar(MachineScalarValue::PointerElementOffset(2)),
            element_size: 8,
            subtract: false,
        },
    ]);

    let ir = ir_of(&module);
    assert!(
        ir.contains("getelementptr i8")
            && (ir.contains("mul i64 2, 8")
                || ir
                    .lines()
                    .any(|line| { line.contains("raw_offset") && line.contains("i64 16") })),
        "PointerElementOffset(2) must be scaled by the 8-byte pointee size:\n{ir}"
    );
}

#[test]
fn local_call_signature_cannot_relabel_machine_result_as_i64() {
    let mut module = values_module();
    let machine_result = LirType::MachineScalar(MachineScalarKind::ForeignCallbackStatus);
    let mut adapter_blocks = Arena::default();
    let adapter_entry = adapter_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![],
        terminator: Terminator::Return {
            value: Some(Value::MachineScalar(
                MachineScalarValue::ForeignCallbackStatus(
                    scoop_lir::ForeignCallbackStatus::Returned,
                ),
            )),
        },
    });
    module.functions.push(Function {
        callable_body: callable_body_at(file!(), line!()),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(vec![], machine_result),
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps: Arena::default(),
        blocks: adapter_blocks,
        entry: adapter_entry,
    });

    let caller = &mut module.functions[0];
    let out = caller.temps.alloc(Temp { ty: LirType::I64 });
    let forged = direct_site(
        &mut caller.call_targets,
        TestCallProtocol::Managed {
            safepoint: 99,
            destination: managed_local(1),
        },
        vec![],
        (LirType::I64, RefScan::None),
        out,
        vec![],
    );
    caller.blocks[caller.entry]
        .instructions
        .push(Instruction::Call { site: forged });

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("a local call must use its declaration's logical result domain");
    assert!(
        error.0.contains("typed local call")
            && error.0.contains(module.functions[1].symbol())
            && error.0.contains("machine<foreign-callback-status>"),
        "unexpected error: {error}"
    );
}

#[test]
fn dispatch_signature_rejects_machine_scalar_domains() {
    let mut module = values_module();
    let function = &mut module.functions[0];
    let slot = function
        .call_targets
        .dispatch_slots
        .alloc_managed(DispatchSlot {
            kind: DispatchKind::Virtual,
            index: 0,
        });
    let destination = scoop_lir::ManagedCallDestination::dispatch(
        Value::NullPointer(PointerKind::Metadata),
        slot,
    );
    let out = function.temps.alloc(Temp {
        ty: LirType::MachineScalar(MachineScalarKind::EnumTag),
    });
    let call = direct_site(
        &mut function.call_targets,
        TestCallProtocol::Managed {
            safepoint: 99,
            destination,
        },
        vec![],
        (
            LirType::MachineScalar(MachineScalarKind::EnumTag),
            RefScan::None,
        ),
        out,
        vec![],
    );
    function.blocks[function.entry]
        .instructions
        .push(Instruction::Call { site: call });
    refresh_test_safepoints(function);

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("dynamic dispatch has no authoritative machine-scalar signature");
    assert!(
        error.0.contains("dispatch signature") && error.0.contains("machine scalar"),
        "unexpected error: {error}"
    );
}

#[test]
fn scoop_extern_rejects_machine_scalar_artifact_abi() {
    let mut module = values_module();
    module
        .extern_functions
        .alloc_scoop(scoop_lir::ScoopExternFunction {
            identity: scoop_lir::ExternFunctionIdentity {
                source_name: "foreignMachine".to_string(),
                native_symbol: "foreign_machine".to_string(),
                library: "fixture".to_string(),
                calling_convention: scoop_lir::CallingConvention::Cdecl,
            },
            gc_effect: GcEffect::NoGc,
            signature: plain_scoop_signature(
                vec![LirType::MachineScalar(MachineScalarKind::EnumTag)],
                LirType::I64,
            ),
        });

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("Scoop artifact ABI must not expose compiler-only scalar domains");
    assert!(
        error.0.contains("Scoop extern `foreignMachine`") && error.0.contains("machine scalar"),
        "unexpected error: {error}"
    );
}

#[test]
fn runtime_call_signature_cannot_relabel_a_machine_result() {
    let mut module = values_module();
    let function = &mut module.functions[0];
    let out = function.temps.alloc(Temp {
        ty: LirType::MachineScalar(MachineScalarKind::EnumTag),
    });
    let call = direct_site(
        &mut function.call_targets,
        TestCallProtocol::NoGc {
            destination: no_gc_runtime(scoop_lir::NoGcRuntimeFunction::GcStats),
        },
        vec![],
        (
            LirType::MachineScalar(MachineScalarKind::EnumTag),
            RefScan::None,
        ),
        out,
        vec![],
    );
    function.blocks[function.entry]
        .instructions
        .push(Instruction::Call { site: call });
    refresh_test_safepoints(function);

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("runtime calls must use their closed logical signature");
    assert!(
        error.0.contains("closed runtime ABI") && error.0.contains("scoop_rt_gc_stats"),
        "unexpected error: {error}"
    );
}

#[test]
fn string_compare_runtime_contract_returns_canonical_long_i64() {
    let mut module = values_module();
    let left = module.globals.iter().next().expect("left string").0;
    let right = module.globals.iter().nth(1).expect("right string").0;
    let function = &mut module.functions[0];
    let out = function.temps.alloc(Temp { ty: LirType::I64 });
    let call = direct_site(
        &mut function.call_targets,
        TestCallProtocol::NoGc {
            destination: no_gc_runtime(scoop_lir::NoGcRuntimeFunction::StringCompare),
        },
        vec![MANAGED_PTR, MANAGED_PTR],
        (LirType::I64, RefScan::None),
        out,
        vec![Value::Global(left), Value::Global(right)],
    );
    function.blocks[function.entry]
        .instructions
        .push(Instruction::Call { site: call });

    let ir = ir_of(&module);
    assert!(
        ir.contains("declare i64 @scoop_rt_string_compare(ptr addrspace(1), ptr addrspace(1))"),
        "String.compareTo runtime boundary must preserve canonical Long width:\n{ir}"
    );
}

#[test]
fn dispatch_table_cannot_hide_a_machine_scalar_local_signature() {
    let mut module = values_module();
    let function_id = scoop_lir::LocalFunctionId::from_u32(module.functions.len() as u32);
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![],
        terminator: Terminator::Return {
            value: Some(Value::MachineScalar(
                MachineScalarValue::ForeignCallbackStatus(
                    scoop_lir::ForeignCallbackStatus::Returned,
                ),
            )),
        },
    });
    module.functions.push(Function {
        callable_body: callable_body_at(file!(), line!()),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(
            vec![],
            LirType::MachineScalar(MachineScalarKind::ForeignCallbackStatus),
        ),
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps: Arena::default(),
        blocks,
        entry,
    });
    module
        .meta
        .type_descriptors
        .iter_mut()
        .next()
        .expect("values module has the String descriptor")
        .1
        .vtable
        .slots_mut()
        .push(DispatchEntry {
            callable: CallableRef::Local(function_id),
        });

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("dispatch metadata must not expose a machine-scalar local function");
    assert!(
        error.0.contains(&format!(
            "dispatches to local function @{}",
            module.functions[function_id.into_u32() as usize].symbol()
        )) && error.0.contains("machine scalar"),
        "unexpected error: {error}"
    );
}

#[test]
fn foreign_callback_operation_rejects_machine_scalar_operand_before_llvm_cast() {
    let mut module = values_module();
    let family = super::c_layout::foreign_callback_family(&mut module);
    let function = &mut module.functions[0];
    function.blocks[function.entry]
        .instructions
        .push(Instruction::ForeignCallbackOperation(
            scoop_lir::ForeignCallbackOperation::Release {
                family,
                callback: Value::MachineScalar(MachineScalarValue::EnumTag(0)),
            },
        ));

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("callback operations must reject a machine scalar without panicking");
    assert!(
        error.0.contains("foreign callback operation") && error.0.contains("machine<enum-tag>"),
        "unexpected error: {error}"
    );
}

#[test]
fn statepoint_plan_cannot_publish_a_machine_scalar_as_a_managed_root() {
    let mut module = values_module();
    let function = &mut module.functions[0];
    let local = function.locals.alloc(Local {
        name: "machine_root".to_string(),
        ty: LirType::MachineScalar(MachineScalarKind::EnumTag),
    });
    let mut call = void_site(
        &mut function.call_targets,
        TestCallProtocol::Managed {
            safepoint: 99,
            destination: managed_runtime(scoop_lir::ManagedRuntimeFunction::GcCollect),
        },
        vec![],
        vec![],
    );
    set_managed_live(
        &mut call,
        statepoint_live(vec![statepoint_value(
            scoop_lir::CallerRootSource::Local(local),
            LirType::MachineScalar(MachineScalarKind::EnumTag),
            &[0],
        )]),
    );
    function.blocks[function.entry]
        .instructions
        .push(Instruction::Call { site: call });
    refresh_test_safepoints(function);

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("machine scalars cannot be forged into address-space-1 roots");
    assert!(
        error.0.contains("managed call root plan")
            && error
                .0
                .contains("has 1 entries, expected 0 complete entries"),
        "unexpected error: {error}"
    );
}
