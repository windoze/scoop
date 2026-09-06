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

fn module_with_nonterminal_invoke_and_invalid_edges() -> Module {
    let mut module = exceptions_module();
    let invalid = scoop_lir::BlockId::from_raw(99.into());
    let mut corrupted = false;
    'functions: for function in &mut module.functions {
        for (_, block) in function.blocks.iter_mut() {
            let Some(Instruction::Invoke { site }) = block.instructions.last_mut() else {
                continue;
            };
            match site {
                scoop_lir::InvokeSite::Managed(site) => {
                    site.normal = invalid;
                    site.unwind = invalid;
                }
                scoop_lir::InvokeSite::NoGc(site) => {
                    site.normal = invalid;
                    site.unwind = invalid;
                }
            }
            block.instructions.push(Instruction::EndCatch);
            corrupted = true;
            break 'functions;
        }
    }
    assert!(corrupted, "EH fixture must have an invoke");
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

#[test]
fn root_plan_validation_rejects_nonterminal_invoke_before_indexing_its_edges() {
    let module = module_with_nonterminal_invoke_and_invalid_edges();
    assert_public_entries_reject(
        &module,
        "invoke @scoop.eh_test: must be the last instruction of block entry",
        "nonterminal_invoke",
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
    let signature = scoop_signature(
        &module.structs,
        &module.enums,
        vec![LirType::Struct(callback)],
        LirType::Void,
    );
    let function = &mut module.functions[0];
    let parameter = function.signature.logical_argument_count() as u32;
    function.signature = signature;
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
        signature: plain_scoop_signature(
            vec![MANAGED_PTR, RAW_PTR, RAW_PTR, RAW_PTR],
            LirType::MachineScalar(MachineScalarKind::ForeignCallbackStatus),
        ),
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
    let signature = plain_scoop_signature(vec![MANAGED_PTR], LirType::Void);
    let function = &mut module.functions[0];
    let closure_parameter = function.signature.logical_argument_count() as u32;
    function.signature = signature;
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
    let signature = scoop_signature(
        &module.structs,
        &module.enums,
        vec![LirType::Struct(callback_struct)],
        LirType::Void,
    );
    let function = &mut module.functions[0];
    let callback_parameter = function.signature.logical_argument_count() as u32;
    function.signature = signature;
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

fn root_plan_test_module(
    functions: Vec<Function>,
    extern_functions: scoop_lir::ExternFunctions,
    entry_symbol: &str,
) -> Module {
    Module {
        globals: Arena::new(),
        initialization_units: Arena::new(),
        structs: scoop_lir::StructDefs::default(),
        enums: scoop_lir::EnumDefs::default(),
        extern_functions,
        native_globals: Arena::new(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::new(),
        foreign_callback_families: Arena::new(),
        foreign_callback_bridges: Arena::new(),
        functions,
        entry_symbol: entry_symbol.to_string(),
        meta: string_metadata(),
    }
}

fn managed_indirect_argument_root_module() -> Module {
    let aggregate = LirType::Aggregate(vec![MANAGED_PTR, LirType::I64, LirType::I64]);
    let abi_value = abi_value_with_layout(aggregate.clone(), 24, 8, RefScan::References(vec![0]));
    let callee_signature = scoop_lir::ScoopAbiSignature::new(
        vec![scoop_lir::AbiArgument::Indirect(abi_value.clone())],
        scoop_lir::AbiReturn::UnitVoid,
        scoop_lir::CallingConvention::Cdecl,
    );
    let mut callee_blocks = Arena::new();
    let callee_entry = callee_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return { value: None },
    });
    let callee = Function {
        gc_effect: GcEffect::Managed,
        symbol: "scoop.root_plan_indirect_callee".to_string(),
        signature: callee_signature,
        call_targets: CallTargets::default(),
        locals: Arena::new(),
        temps: Arena::new(),
        blocks: callee_blocks,
        entry: callee_entry,
    };

    let mut locals = Arena::new();
    let argument = locals.alloc(Local {
        name: "indirect_argument".to_string(),
        ty: aggregate.clone(),
    });
    let mut temps = Arena::new();
    let value = temps.alloc(Temp {
        ty: aggregate.clone(),
    });
    let mut targets = CallTargets::default();
    let signature = targets.void_signatures.alloc(VoidCallSignature::new(
        vec![scoop_lir::AbiArgument::Indirect(abi_value.clone())],
        scoop_lir::CallingConvention::Cdecl,
    ));
    let storage = scoop_lir::AbiArgumentStorage::new(argument, &locals[argument].ty, &abi_value)
        .expect("test indirect argument has exact storage");
    let site = protocol_site(
        &mut targets,
        TestCallProtocol::Managed {
            safepoint: 700,
            destination: managed_local(0),
        },
        TestTypedCall::Void {
            signature,
            args: vec![scoop_lir::AbiCallArgument::Indirect(storage)],
        },
    );
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::MakeAggregate {
                out: value,
                elements: vec![Value::Param(0), signed64(1), signed64(2)],
            },
            Instruction::Store {
                local: argument,
                value: Value::Temp(value),
            },
            Instruction::Call { site },
        ],
        terminator: Terminator::Return { value: None },
    });
    let caller = Function {
        gc_effect: GcEffect::Managed,
        symbol: "scoop.root_plan_indirect_caller".to_string(),
        signature: plain_scoop_signature(vec![MANAGED_PTR], LirType::Void),
        call_targets: targets,
        locals,
        temps,
        blocks,
        entry,
    };
    root_plan_test_module(
        vec![callee, caller],
        scoop_lir::ExternFunctions::default(),
        "scoop.root_plan_indirect_caller",
    )
}

fn native_borrowed_root_module(scan: RefScan) -> Module {
    let mut extern_functions = scoop_lir::ExternFunctions::default();
    let external = extern_functions.alloc_scoop(scoop_lir::ScoopExternFunction {
        identity: scoop_lir::ExternFunctionIdentity {
            source_name: "rootPlanBorrowed".to_string(),
            native_symbol: "root_plan_borrowed".to_string(),
            library: "fixture".to_string(),
            calling_convention: scoop_lir::CallingConvention::Cdecl,
        },
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(Vec::new(), LirType::Void),
    });
    let mut targets = CallTargets::default();
    let mut site = void_site(
        &mut targets,
        TestCallProtocol::NativeBorrowed {
            safepoint: 701,
            destination: scoop_lir::NativeBorrowedCallDestination::extern_function(external),
            result: NativeBorrowedResultRoot::GcFree,
        },
        Vec::new(),
        Vec::new(),
    );
    let CallSite::NativeBorrowed(native_site) = &mut site else {
        unreachable!("test constructs a native-borrowed call")
    };
    native_site.roots = scoop_lir::NativeBorrowedRootSet::new(vec![scoop_lir::CallerRoot {
        source: scoop_lir::CallerRootSource::Param(0),
        scan: scoop_lir::NonEmptyRefScan::new(scan).expect("test root scan is non-empty"),
    }]);
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![Instruction::Call { site }],
        terminator: Terminator::Return {
            value: Some(Value::Param(0)),
        },
    });
    let caller = Function {
        gc_effect: GcEffect::Managed,
        symbol: "scoop.root_plan_borrowed_caller".to_string(),
        signature: plain_scoop_signature(vec![MANAGED_PTR], MANAGED_PTR),
        call_targets: targets,
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    };
    root_plan_test_module(
        vec![caller],
        extern_functions,
        "scoop.root_plan_borrowed_caller",
    )
}

fn managed_invoke_root_module(normal_live: bool, unwind_live: bool) -> Module {
    let mut callee_blocks = Arena::new();
    let callee_entry = callee_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return { value: None },
    });
    let callee = Function {
        gc_effect: GcEffect::Managed,
        symbol: "scoop.root_plan_invoke_callee".to_string(),
        signature: plain_scoop_signature(Vec::new(), LirType::Void),
        call_targets: CallTargets::default(),
        locals: Arena::new(),
        temps: Arena::new(),
        blocks: callee_blocks,
        entry: callee_entry,
    };

    let mut targets = CallTargets::default();
    let call = void_site(
        &mut targets,
        TestCallProtocol::Managed {
            safepoint: 702,
            destination: managed_local(0),
        },
        Vec::new(),
        Vec::new(),
    );
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Unreachable,
    });
    let normal = blocks.alloc(BasicBlock {
        name: "normal".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return {
            value: Some(Value::Param(0)),
        },
    });
    let unwind = blocks.alloc(BasicBlock {
        name: "unwind".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return {
            value: Some(Value::Param(0)),
        },
    });
    let mut invoke = managed_invoke(call, normal, unwind);
    let scoop_lir::InvokeSite::Managed(site) = &mut invoke else {
        unreachable!("test constructs a managed invoke")
    };
    site.roots = scoop_lir::ExceptionalRootSet::new(vec![scoop_lir::ExceptionalRoot {
        root: scoop_lir::CallerRoot {
            source: scoop_lir::CallerRootSource::Param(0),
            scan: scoop_lir::NonEmptyRefScan::new(RefScan::References(vec![0]))
                .expect("managed pointer has one root"),
        },
        normal_live,
        unwind_live,
    }]);
    blocks[entry] = BasicBlock {
        name: "entry".to_string(),
        instructions: vec![Instruction::Invoke { site: invoke }],
        terminator: Terminator::Br(normal),
    };
    let caller = Function {
        gc_effect: GcEffect::Managed,
        symbol: "scoop.root_plan_invoke_caller".to_string(),
        signature: plain_scoop_signature(vec![MANAGED_PTR], MANAGED_PTR),
        call_targets: targets,
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    };
    root_plan_test_module(
        vec![callee, caller],
        scoop_lir::ExternFunctions::default(),
        "scoop.root_plan_invoke_caller",
    )
}

#[test]
fn root_plan_validation_rejects_omitted_indirect_argument_root() {
    let module = managed_indirect_argument_root_module();
    assert_module_validation_error(
        &module,
        "root plan has 0 entries, expected 1 complete entries",
    );
}

#[test]
fn root_plan_validation_rejects_noncanonical_caller_scan() {
    let module = native_borrowed_root_module(RefScan::References(vec![8]));
    assert_module_validation_error(
        &module,
        "root param0 has scan refs[8], expected canonical refs[0]",
    );
}

#[test]
fn root_plan_validation_rejects_incorrect_invoke_edge_flags() {
    let module = managed_invoke_root_module(true, false);
    assert_module_validation_error(
        &module,
        "root param0 has edge flags normal_live=true/unwind_live=false, expected true/true",
    );
}
