use super::*;

fn module_with_invalid_invoke_unwind() -> Module {
    let mut module = exceptions_module();
    let invalid = scoop_lir::BlockId::from_raw(99.into());
    let mut first_safepoint = None;
    let mut duplicated_safepoint = false;
    'functions: for function in &mut module.functions {
        for (_, block) in function.blocks.iter_mut() {
            let Some(Instruction::Invoke { site }) = block.instructions.last_mut() else {
                continue;
            };
            match site {
                scoop_lir::InvokeSite::Managed(site) => {
                    if let Some(first) = first_safepoint {
                        site.safepoint = first;
                        duplicated_safepoint = true;
                        break 'functions;
                    }
                    first_safepoint = Some(site.safepoint);
                    site.unwind = invalid;
                }
                scoop_lir::InvokeSite::NoGc(site) => site.unwind = invalid,
            }
        }
    }
    assert!(
        duplicated_safepoint,
        "EH fixture must have two managed invokes"
    );
    module
}

fn assert_validation_error<T>(
    outcome: std::thread::Result<Result<T, CodegenError>>,
    expected: &str,
) {
    match outcome {
        Ok(Err(error)) => assert!(
            error.0.contains(expected),
            "unexpected validation error: {error}"
        ),
        Ok(Ok(_)) => panic!("malformed LIR unexpectedly passed public codegen preflight"),
        Err(_) => panic!("public codegen entry panicked before returning its validation error"),
    }
}

fn assert_public_entries_reject(module: &Module, expected: &str, fixture: &str) {
    let profile = host_profile();
    let output = std::env::temp_dir().join(format!(
        "scoop_codegen_{fixture}_preflight_test_{}.o",
        std::process::id()
    ));
    std::fs::remove_file(&output).ok();

    assert_validation_error(
        std::panic::catch_unwind(|| emit_object(module, &output, profile)),
        expected,
    );
    assert!(
        !output.exists(),
        "failed preflight must not write an object"
    );
    assert_validation_error(
        std::panic::catch_unwind(|| render_llvm_ir(module, profile)),
        expected,
    );
    assert_validation_error(
        std::panic::catch_unwind(|| c_bridge_source(module)),
        expected,
    );
    assert_validation_error(
        std::panic::catch_unwind(|| c_layout_assertions(module)),
        expected,
    );
}

fn assert_module_validation_error(module: &Module, expected: &str) {
    assert_validation_error(
        std::panic::catch_unwind(|| crate::validation::validate_module(module)),
        expected,
    );
}

#[test]
fn public_codegen_entries_validate_before_manifests_and_eh_edges() {
    let module = module_with_invalid_invoke_unwind();
    assert_public_entries_reject(
        &module,
        "variant control-flow validation in @scoop.eh_test reached invalid block 99",
        "eh_edge",
    );
}

fn invalid_temp() -> scoop_lir::TempId {
    scoop_lir::TempId::from_raw(99.into())
}

fn module_with_invalid_enum_wrap_output() -> Module {
    let mut module = enum_module();
    let function = &mut module.functions[0];
    let Instruction::EnumWrap { out, .. } = &mut function.blocks[function.entry].instructions[0]
    else {
        panic!("enum fixture starts with an enum wrap")
    };
    *out = invalid_temp();
    module
}

fn module_with_invalid_callback_state_output() -> Module {
    let mut module = values_module();
    let family = super::c_layout::foreign_callback_family(&mut module);
    let callback = module.foreign_callback_families[family].callback;
    let function = &mut module.functions[0];
    let parameter = function.params.len() as u32;
    function.params.push(LirType::Struct(callback));
    function.blocks[function.entry]
        .instructions
        .push(Instruction::ForeignCallbackOperation(
            scoop_lir::ForeignCallbackOperation::State {
                family,
                out: invalid_temp(),
                callback: Value::Param(parameter),
            },
        ));
    module
}

fn callback_adapter(symbol: &str) -> Function {
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return {
            value: Some(Value::MachineScalar(
                MachineScalarValue::ForeignCallbackStatus(
                    scoop_lir::ForeignCallbackStatus::Returned,
                ),
            )),
        },
    });
    Function {
        gc_effect: GcEffect::Managed,
        symbol: symbol.to_string(),
        params: vec![MANAGED_PTR, RAW_PTR, RAW_PTR, RAW_PTR],
        return_ty: LirType::MachineScalar(MachineScalarKind::ForeignCallbackStatus),
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps: Arena::default(),
        blocks,
        entry,
    }
}

#[derive(Clone, Copy)]
enum CallbackRegistrationCorruption {
    Result,
    ClosureParameter,
    ClosureTemporary,
    ClosureGlobal,
}

fn malformed_callback_registration(corruption: CallbackRegistrationCorruption) -> Module {
    let mut module = values_module();
    let family = super::c_layout::foreign_callback_family(&mut module);
    let callback = module.foreign_callback_families[family].callback;
    let mode = module.foreign_callback_families[family].modes.reusable();
    let adapter_symbol = "scoop.invalid_id_callback_adapter";
    module.functions.push(callback_adapter(adapter_symbol));
    let bridge = module
        .foreign_callback_bridges
        .alloc(scoop_lir::ForeignCallbackBridge {
            family,
            adapter_symbol: adapter_symbol.to_string(),
            trampoline_symbol: "scoop.invalid_id_callback_trampoline".to_string(),
            signature_symbol: "scoop.invalid_id_callback_signature".to_string(),
            params: vec![scoop_lir::CType::DataPointer {
                pointee: scoop_lir::CDataPointee::OpaqueVoid,
                storage: scoop_lir::CDataPointerStorage::Direct,
            }],
            return_type: scoop_lir::CReturnType::Void,
            context_index: 0,
            mode,
        });
    let function = &mut module.functions[0];
    let closure_parameter = function.params.len() as u32;
    function.params.push(MANAGED_PTR);
    let valid_out = function.temps.alloc(Temp {
        ty: LirType::Struct(callback),
    });
    let out = if matches!(corruption, CallbackRegistrationCorruption::Result) {
        invalid_temp()
    } else {
        valid_out
    };
    let closure = match corruption {
        CallbackRegistrationCorruption::Result => Value::Param(closure_parameter),
        CallbackRegistrationCorruption::ClosureParameter => Value::Param(99),
        CallbackRegistrationCorruption::ClosureTemporary => Value::Temp(invalid_temp()),
        CallbackRegistrationCorruption::ClosureGlobal => {
            Value::Global(scoop_lir::GlobalId::from_raw(99.into()))
        }
    };
    function.blocks[function.entry]
        .instructions
        .push(Instruction::ForeignCallbackRegister {
            out,
            bridge,
            closure,
        });
    module
}

#[derive(Clone, Copy)]
enum CallbackOperationCorruption {
    Result,
    CallbackParameter,
    CallbackTemporary,
    CallbackGlobal,
}

fn malformed_callback_operation(corruption: CallbackOperationCorruption) -> Module {
    let mut module = values_module();
    let family = super::c_layout::foreign_callback_family(&mut module);
    let callback_struct = module.foreign_callback_families[family].callback;
    let state = module.foreign_callback_families[family].states.definition();
    let function = &mut module.functions[0];
    let callback_parameter = function.params.len() as u32;
    function.params.push(LirType::Struct(callback_struct));
    let valid_out = function.temps.alloc(Temp {
        ty: LirType::Enum(state),
    });
    let out = if matches!(corruption, CallbackOperationCorruption::Result) {
        invalid_temp()
    } else {
        valid_out
    };
    let callback = match corruption {
        CallbackOperationCorruption::Result => Value::Param(callback_parameter),
        CallbackOperationCorruption::CallbackParameter => Value::Param(99),
        CallbackOperationCorruption::CallbackTemporary => Value::Temp(invalid_temp()),
        CallbackOperationCorruption::CallbackGlobal => {
            Value::Global(scoop_lir::GlobalId::from_raw(99.into()))
        }
    };
    function.blocks[function.entry]
        .instructions
        .push(Instruction::ForeignCallbackOperation(
            scoop_lir::ForeignCallbackOperation::State {
                family,
                out,
                callback,
            },
        ));
    module
}

#[test]
fn public_codegen_entries_reject_invalid_typed_instruction_ids_without_panicking() {
    let module = module_with_invalid_enum_wrap_output();
    assert_public_entries_reject(
        &module,
        "enum_wrap result references invalid temporary t99 in @scoop.tagged",
        "enum_result_id",
    );

    let module = module_with_invalid_callback_state_output();
    assert_public_entries_reject(
        &module,
        "foreign callback state result references invalid temporary t99 in @scoop_main",
        "callback_result_id",
    );
}

#[test]
fn enum_wrap_rejects_every_out_of_bounds_value_reference_before_type_lookup() {
    let cases = [
        (
            Value::Local(scoop_lir::LocalId::from_raw(99.into())),
            "enum_wrap field 0 references invalid local 99 in @scoop.tagged",
        ),
        (
            Value::Param(99),
            "enum_wrap field 0 references invalid parameter 99 in @scoop.tagged",
        ),
        (
            Value::Temp(invalid_temp()),
            "enum_wrap field 0 references invalid temporary t99 in @scoop.tagged",
        ),
        (
            Value::Global(scoop_lir::GlobalId::from_raw(99.into())),
            "enum_wrap field 0 references invalid global 99 in @scoop.tagged",
        ),
    ];

    for (value, expected) in cases {
        let mut module = enum_module();
        let function = &mut module.functions[0];
        let Instruction::EnumWrap { fields, .. } =
            &mut function.blocks[function.entry].instructions[2]
        else {
            panic!("enum fixture has a one-field enum wrap")
        };
        fields[0] = value;
        assert_module_validation_error(&module, expected);
    }
}

#[test]
fn typed_variant_primitives_reject_invalid_result_and_operand_ids() {
    let mut module = enum_module();
    let shape = module.enums.iter().next().expect("Shape enum").0;
    let variant = module.enums.variant_ref(shape, 0).expect("Dot variant");
    let function = &mut module.functions[0];
    function.blocks[function.entry]
        .instructions
        .push(Instruction::VariantTest {
            out: invalid_temp(),
            operand: Value::Param(0),
            variant,
        });
    assert_module_validation_error(
        &module,
        "variant_test result references invalid temporary t99 in @scoop.tagged",
    );

    let mut module = enum_module();
    let shape = module.enums.iter().next().expect("Shape enum").0;
    let variant = module.enums.variant_ref(shape, 0).expect("Dot variant");
    let function = &mut module.functions[0];
    let out = function.temps.alloc(Temp { ty: LirType::I1 });
    function.blocks[function.entry]
        .instructions
        .push(Instruction::VariantTest {
            out,
            operand: Value::Param(99),
            variant,
        });
    assert_module_validation_error(
        &module,
        "variant_test operand references invalid parameter 99 in @scoop.tagged",
    );

    let mut module = enum_module();
    let shape = module.enums.iter().next().expect("Shape enum").0;
    let variant = module.enums.variant_ref(shape, 1).expect("Circle variant");
    let field = module
        .enums
        .variant_field_ref(variant, 0)
        .expect("Circle field");
    let function = &mut module.functions[0];
    function.blocks[function.entry]
        .instructions
        .push(Instruction::VariantPayloadProject {
            out: invalid_temp(),
            operand: Value::Param(0),
            field,
        });
    assert_module_validation_error(
        &module,
        "variant_payload_project result references invalid temporary t99 in @scoop.tagged",
    );

    let mut module = enum_module();
    let shape = module.enums.iter().next().expect("Shape enum").0;
    let variant = module.enums.variant_ref(shape, 1).expect("Circle variant");
    let field = module
        .enums
        .variant_field_ref(variant, 0)
        .expect("Circle field");
    let function = &mut module.functions[0];
    let out = function.temps.alloc(Temp { ty: LirType::I64 });
    function.blocks[function.entry]
        .instructions
        .push(Instruction::VariantPayloadProject {
            out,
            operand: Value::Temp(invalid_temp()),
            field,
        });
    assert_module_validation_error(
        &module,
        "variant_payload_project operand references invalid temporary t99 in @scoop.tagged",
    );
}

#[test]
fn callback_instructions_reject_invalid_result_and_operand_ids() {
    for (corruption, expected) in [
        (
            CallbackRegistrationCorruption::Result,
            "foreign callback registration result references invalid temporary t99 in @scoop_main",
        ),
        (
            CallbackRegistrationCorruption::ClosureParameter,
            "foreign callback registration closure references invalid parameter 99 in @scoop_main",
        ),
        (
            CallbackRegistrationCorruption::ClosureTemporary,
            "foreign callback registration closure references invalid temporary t99 in @scoop_main",
        ),
        (
            CallbackRegistrationCorruption::ClosureGlobal,
            "foreign callback registration closure references invalid global 99 in @scoop_main",
        ),
    ] {
        let module = malformed_callback_registration(corruption);
        assert_module_validation_error(&module, expected);
    }

    for (corruption, expected) in [
        (
            CallbackOperationCorruption::Result,
            "foreign callback state result references invalid temporary t99 in @scoop_main",
        ),
        (
            CallbackOperationCorruption::CallbackParameter,
            "foreign callback operation callback references invalid parameter 99 in @scoop_main",
        ),
        (
            CallbackOperationCorruption::CallbackTemporary,
            "foreign callback operation callback references invalid temporary t99 in @scoop_main",
        ),
        (
            CallbackOperationCorruption::CallbackGlobal,
            "foreign callback operation callback references invalid global 99 in @scoop_main",
        ),
    ] {
        let module = malformed_callback_operation(corruption);
        assert_module_validation_error(&module, expected);
    }
}
