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

fn assert_scoop_entries_reject(module: Module, expected: &str, fixture: &str) {
    let expected = resolve_function_markers(&module, expected);
    let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new())
        .expect("malformed validation fixture still has a complete strong foundation");
    let profile = host_profile();
    let output = std::env::temp_dir().join(format!(
        "scoop_codegen_{fixture}_preflight_test_{}.o",
        std::process::id()
    ));
    std::fs::remove_file(&output).ok();

    assert_validation_error(
        std::panic::catch_unwind(|| {
            emit_object_set(
                &input,
                &scoop_lir::ConeCoordinate::reserved_single_file(),
                &[scoop_identity::ConeIdentity::CORE],
                scoop_lir::EntryProductionSourceV1::Library,
                &output,
                profile,
            )
        }),
        &expected,
    );
    assert!(
        !output.exists(),
        "failed preflight must not write an object"
    );
    assert_validation_error(
        std::panic::catch_unwind(|| {
            render_llvm_ir_members(
                &input,
                &scoop_lir::ConeCoordinate::reserved_single_file(),
                &[scoop_identity::ConeIdentity::CORE],
                scoop_lir::EntryProductionSourceV1::Library,
                profile,
            )
        }),
        &expected,
    );
}

fn assert_output_validation_error(module: Module, expected: &str) {
    let error = scoop_lir::ConeLirOutput::try_new(module, Vec::new())
        .err()
        .expect("malformed identities must fail at the LIR output boundary");
    assert!(error.to_string().contains(expected), "{error}");
}

fn assert_module_validation_error(module: &Module, expected: &str) {
    let expected = resolve_function_markers(module, expected);
    assert_validation_error(
        std::panic::catch_unwind(|| crate::validation::validate_module(module)),
        &expected,
    );
}

fn resolve_function_markers(module: &Module, expected: &str) -> String {
    module
        .functions
        .iter()
        .enumerate()
        .fold(expected.to_string(), |text, (index, function)| {
            text.replace(
                &format!("{{function:{index}}}"),
                &format!("@{}", function.symbol()),
            )
        })
}

#[test]
fn scoop_codegen_entries_validate_before_manifests_and_eh_edges() {
    let module = module_with_invalid_invoke_unwind();
    assert_scoop_entries_reject(
        module,
        "variant control-flow validation in {function:2} reached invalid block 99",
        "eh_edge",
    );
}

#[test]
fn root_plan_validation_rejects_nonterminal_invoke_before_indexing_its_edges() {
    let module = module_with_nonterminal_invoke_and_invalid_edges();
    assert_scoop_entries_reject(
        module,
        "invoke {function:2}: must be the last instruction of block entry",
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
        callable_body: callable_body(symbol),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(
            vec![MANAGED_PTR, MANAGED_PTR, RAW_PTR, RAW_PTR, RAW_PTR],
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
    let adapter_index = module.functions.len();
    module.functions.push(callback_adapter(adapter_symbol));
    let bridge = module
        .foreign_callback_bridges
        .alloc(scoop_lir::ForeignCallbackBridge {
            application: callback_application(0),
            family,
            adapter: managed_local_function_ref(adapter_index),
            trampoline: callback_trampoline(1, 0),
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
    assert_scoop_entries_reject(
        module,
        "enum_wrap result references invalid temporary t99 in {function:0}",
        "enum_result_id",
    );

    let module = module_with_invalid_callback_state_output();
    assert_scoop_entries_reject(
        module,
        "foreign callback state result references invalid temporary t99 in {function:0}",
        "callback_result_id",
    );
}

#[test]
fn enum_wrap_rejects_every_out_of_bounds_value_reference_before_type_lookup() {
    let cases = [
        (
            Value::Local(scoop_lir::LocalId::from_raw(99.into())),
            "enum_wrap field 0 references invalid local 99 in {function:0}",
        ),
        (
            Value::Param(99),
            "enum_wrap field 0 references invalid parameter 99 in {function:0}",
        ),
        (
            Value::Temp(invalid_temp()),
            "enum_wrap field 0 references invalid temporary t99 in {function:0}",
        ),
        (
            Value::Global(scoop_lir::GlobalId::from_raw(99.into())),
            "enum_wrap field 0 references invalid global 99 in {function:0}",
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
        "variant_test result references invalid temporary t99 in {function:0}",
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
        "variant_test operand references invalid parameter 99 in {function:0}",
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
        "variant_payload_project result references invalid temporary t99 in {function:0}",
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
        "variant_payload_project operand references invalid temporary t99 in {function:0}",
    );
}

#[test]
fn callback_instructions_reject_invalid_result_and_operand_ids() {
    for (corruption, expected) in [
        (
            CallbackRegistrationCorruption::Result,
            "foreign callback registration result references invalid temporary t99 in {function:0}",
        ),
        (
            CallbackRegistrationCorruption::ClosureParameter,
            "foreign callback registration closure references invalid parameter 99 in {function:0}",
        ),
        (
            CallbackRegistrationCorruption::ClosureTemporary,
            "foreign callback registration closure references invalid temporary t99 in {function:0}",
        ),
        (
            CallbackRegistrationCorruption::ClosureGlobal,
            "foreign callback registration closure references invalid global 99 in {function:0}",
        ),
    ] {
        let module = malformed_callback_registration(corruption);
        assert_module_validation_error(&module, expected);
    }

    for (corruption, expected) in [
        (
            CallbackOperationCorruption::Result,
            "foreign callback state result references invalid temporary t99 in {function:0}",
        ),
        (
            CallbackOperationCorruption::CallbackParameter,
            "foreign callback operation callback references invalid parameter 99 in {function:0}",
        ),
        (
            CallbackOperationCorruption::CallbackTemporary,
            "foreign callback operation callback references invalid temporary t99 in {function:0}",
        ),
        (
            CallbackOperationCorruption::CallbackGlobal,
            "foreign callback operation callback references invalid global 99 in {function:0}",
        ),
    ] {
        let module = malformed_callback_operation(corruption);
        assert_module_validation_error(&module, expected);
    }
}

fn root_plan_test_module(
    functions: Vec<Function>,
    extern_functions: scoop_lir::ExternFunctions,
    entry_index: usize,
) -> Module {
    let mut module = Module {
        release_hooks: Default::default(),
        cone: scoop_identity::ConeIdentity::SINGLE_FILE,
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
        output: scoop_lir::LirOutput::Executable {
            entry: managed_function_ref(entry_index),
        },
        meta: string_metadata(),
    };
    refresh_module_safepoints(&mut module);
    module
}

#[test]
fn callable_runtime_scan_trees_are_emitted_as_closed_strong_atoms() {
    let mut module = managed_poll_test_module();
    let element = scoop_lir::NonEmptyRefScan::new(RefScan::References(vec![8])).unwrap();
    module.functions[0]
        .call_targets
        .root_scans
        .alloc(RefScan::Sequence(vec![
            RefScan::None,
            RefScan::References(vec![0, 16]),
            RefScan::Array {
                length_offset: 24,
                first_element_offset: 32,
                stride: std::num::NonZeroU64::new(8).unwrap(),
                element: Box::new(element),
            },
        ]));
    let plans = scoop_lir::StrongCallableRuntimeScanPlanSetV1::from_module(&module).unwrap();
    let callable = plans
        .callable(module.functions[0].callable_body.id())
        .unwrap();
    assert_eq!(callable.atoms().len(), 4);

    let foundation = scoop_lir::ConeLirFoundation::from_module(&module).unwrap();
    let surface = scoop_lir::ObjectSymbolSurfaceV1::from_foundation(&foundation).unwrap();
    let machine = host_target_machine().unwrap();
    let context = inkwell::context::Context::create();
    let llvm = emit_llvm_module(&context, &module, &machine, host_profile()).unwrap();
    let ir = llvm.print_to_string().to_string();
    for atom in callable.atoms() {
        let boundary = surface
            .plans()
            .iter()
            .flat_map(|plan| plan.atom_boundaries())
            .find(|boundary| boundary.atom() == atom.atom())
            .unwrap();
        assert!(
            ir.contains(boundary.start().symbol().as_str()),
            "missing runtime-scan start boundary for {}",
            atom.atom()
        );
        assert!(
            ir.contains(boundary.end().symbol().as_str()),
            "missing runtime-scan end boundary for {}",
            atom.atom()
        );
    }
    assert!(!ir.contains(".root_scan."), "{ir}");
    assert!(!ir.contains(".native."), "{ir}");
    assert!(!ir.contains(".invoke."), "{ir}");
}

fn managed_poll_test_module() -> Module {
    let mut call_targets = CallTargets::default();
    let signature = call_targets.void_signatures.alloc(VoidCallSignature::new(
        Vec::new(),
        scoop_lir::CallingConvention::Cdecl,
    ));
    let target = call_targets
        .managed_targets
        .void
        .alloc(scoop_lir::CallTarget {
            destination: scoop_lir::ManagedCallDestination::runtime(
                scoop_lir::ManagedRuntimeFunction::Safepoint,
            ),
            signature,
        });
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![Instruction::ManagedPoll {
            site: scoop_lir::ManagedPollSite {
                target,
                safepoint: test_safepoint(703),
                live: scoop_lir::StatepointLiveSet::default(),
            },
        }],
        terminator: Terminator::Return { value: None },
    });
    let function = Function {
        callable_body: callable_body_at(file!(), line!()),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(Vec::new(), LirType::Void),
        call_targets,
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    };
    root_plan_test_module(vec![function], scoop_lir::ExternFunctions::default(), 0)
}

fn managed_poll_reference(function: &Function) -> scoop_lir::SafepointSiteRef {
    match &function.blocks[function.entry].instructions[0] {
        Instruction::ManagedPoll { site } => site.safepoint,
        _ => panic!("managed poll fixture starts with a poll"),
    }
}

#[test]
fn safepoint_validation_rejects_a_missing_identity_record() {
    let mut module = managed_poll_test_module();
    module.functions[0].safepoints = scoop_lir::SafepointIdentities::default();

    assert_module_validation_error(&module, "references missing safepoint site 702");
}

#[test]
fn safepoint_validation_rejects_an_identity_owned_by_another_body() {
    let mut module = managed_poll_test_module();
    let function = &mut module.functions[0];
    let reference = managed_poll_reference(function);
    let identity = scoop_lir::SafepointIdentity::new(
        callable_body("another_safepoint_owner").id(),
        scoop_lir::SafepointSiteRole::ManagedPoll,
        0,
    )
    .unwrap();
    function.safepoints =
        scoop_lir::SafepointIdentities::checked(vec![(reference, identity)]).unwrap();

    assert_module_validation_error(&module, "belongs to another callable body");
}

#[test]
fn duplicate_type_descriptor_identity_is_rejected() {
    let mut module = values_module();
    let identity = module
        .meta
        .type_descriptors
        .iter()
        .next()
        .expect("values fixture has the String descriptor")
        .1
        .identity
        .clone();
    module.meta.type_descriptors.alloc(TypeDescriptor {
        release_policy: Default::default(),
        relations: Default::default(),
        diagnostic_name: "DuplicateString".to_string(),
        identity,
        instance_layout: layout_identity(
            "String",
            scoop_identity::RepresentationRole::ManagedObject,
        ),
        instance_shape: TypeInstanceShapeV1::inline_bytes(
            scoop_lir::LirTargetProfile::DARWIN_AARCH64,
        )
        .unwrap(),
        inline_scan: scoop_lir::TypeDescriptorInlineScanV1::Null,
        parent: None,
        vtable: vtable("String", Vec::new()),
        itables: Vec::new(),
    });

    assert_output_validation_error(module, "duplicate persistent symbol request");
}

#[test]
fn type_descriptor_validation_rejects_an_empty_diagnostic_name() {
    let mut module = values_module();
    module
        .meta
        .type_descriptors
        .iter_mut()
        .next()
        .expect("values fixture has the String descriptor")
        .1
        .diagnostic_name
        .clear();

    assert_module_validation_error(&module, "has an empty canonical diagnostic name");
}

#[test]
fn type_descriptor_validation_rejects_a_foreign_instance_layout() {
    let mut module = values_module();
    let descriptor = module
        .meta
        .type_descriptors
        .iter_mut()
        .next()
        .expect("values fixture has the String descriptor")
        .1;
    descriptor.instance_layout = layout_identity(
        "NotString",
        scoop_identity::RepresentationRole::ManagedObject,
    );

    assert_module_validation_error(
        &module,
        "carries an instance layout for another exact type, target, representation, or scan role",
    );
}

#[test]
fn dispatch_table_validation_rejects_a_vtable_for_another_exact_type() {
    let mut module = values_module();
    module
        .meta
        .type_descriptors
        .iter_mut()
        .next()
        .expect("values fixture has the String descriptor")
        .1
        .vtable = vtable("NotString", Vec::new());

    assert_module_validation_error(
        &module,
        "carries a vtable identity for another exact type or table role",
    );
}

#[test]
fn dispatch_table_validation_rejects_an_itable_for_another_interface() {
    let mut module = super::objects::classes_module();
    let descriptor_id = |name: &str| {
        module
            .meta
            .type_descriptors
            .iter()
            .find_map(|(id, descriptor)| (descriptor.diagnostic_name == name).then_some(id))
            .unwrap_or_else(|| panic!("missing test descriptor {name}"))
    };
    let describable = descriptor_id("Describable");
    let point = descriptor_id("Point");
    let slots = module.meta.type_descriptors[point].itables[0]
        .slots()
        .to_vec();
    module.meta.type_descriptors[point].itables[0] = itable(
        "Point",
        "Shape",
        TypeDescriptorRef::Local(describable),
        slots,
    );

    assert_module_validation_error(
        &module,
        "itable identity and interface descriptor identify different exact types",
    );
}

#[test]
fn layout_validation_rejects_two_physical_layouts_for_one_identity() {
    let mut module = values_module();
    let identity = module
        .meta
        .layouts
        .iter()
        .next()
        .expect("values fixture has the String layout")
        .1
        .identity
        .clone();
    module.meta.layouts.alloc(Layout {
        identity,
        name: "DuplicateString".to_string(),
        size: 24,
        align: 8,
        fields: Vec::new(),
        c_layout: None,
        interior_mutable: false,
        kind: LayoutKind::Intrinsic(scoop_lir::IntrinsicTypeRepresentation::String),
    });

    assert_output_validation_error(module, "duplicate layout identity");
}

#[test]
fn static_storage_validation_rejects_two_globals_for_one_identity() {
    let mut module = values_module();
    for _ in 0..2 {
        module.globals.alloc(Global {
            address_kind: PointerKind::Raw,
            scan: RefScan::None,
            init: GlobalInit::Storage {
                identity: static_storage_identity("duplicateStorage"),
                layout: layout_identity(
                    "duplicateStorage",
                    scoop_identity::RepresentationRole::ManagedValue,
                )
                .into(),
                ty: LirType::I64,
                initial_state: LirStaticInitialState::EncodedStaticValue {
                    payload: LirConstantImage::Integer(scoop_lir::LirIntegerConstant::Signed64(0)),
                },
            },
        });
    }

    assert_output_validation_error(module, "duplicate static storage identity");
}

#[test]
fn safepoint_validation_rejects_a_role_that_disagrees_with_the_instruction() {
    let mut module = managed_poll_test_module();
    let function = &mut module.functions[0];
    let reference = managed_poll_reference(function);
    let identity = scoop_lir::SafepointIdentity::new(
        function.callable_body.id(),
        scoop_lir::SafepointSiteRole::ManagedCall,
        0,
    )
    .unwrap();
    function.safepoints =
        scoop_lir::SafepointIdentities::checked(vec![(reference, identity)]).unwrap();

    assert_module_validation_error(&module, "has role ManagedCall, expected ManagedPoll");
}

#[test]
fn safepoint_validation_rejects_a_noncanonical_role_ordinal() {
    let mut module = managed_poll_test_module();
    let function = &mut module.functions[0];
    let reference = managed_poll_reference(function);
    let identity = scoop_lir::SafepointIdentity::new(
        function.callable_body.id(),
        scoop_lir::SafepointSiteRole::ManagedPoll,
        1,
    )
    .unwrap();
    function.safepoints =
        scoop_lir::SafepointIdentities::checked(vec![(reference, identity)]).unwrap();

    assert_module_validation_error(&module, "ManagedPoll ordinal 1, expected 0");
}

#[test]
fn safepoint_validation_rejects_an_unreferenced_identity_record() {
    let mut module = managed_poll_test_module();
    let function = &mut module.functions[0];
    let reference = managed_poll_reference(function);
    let extra_reference = scoop_lir::SafepointSiteRef::from_u32(800);
    let first = scoop_lir::SafepointIdentity::new(
        function.callable_body.id(),
        scoop_lir::SafepointSiteRole::ManagedPoll,
        0,
    )
    .unwrap();
    let extra = scoop_lir::SafepointIdentity::new(
        function.callable_body.id(),
        scoop_lir::SafepointSiteRole::ManagedPoll,
        1,
    )
    .unwrap();
    function.safepoints =
        scoop_lir::SafepointIdentities::checked(vec![(reference, first), (extra_reference, extra)])
            .unwrap();

    assert_module_validation_error(
        &module,
        "has 1 referenced safepoints but 2 identity records",
    );
}

#[test]
fn safepoint_validation_rejects_a_reference_reused_by_two_instructions() {
    let mut module = managed_poll_test_module();
    let function = &mut module.functions[0];
    let (target, reference) = match &function.blocks[function.entry].instructions[0] {
        Instruction::ManagedPoll { site } => (site.target, site.safepoint),
        _ => panic!("managed poll fixture starts with a poll"),
    };
    function.blocks[function.entry]
        .instructions
        .push(Instruction::ManagedPoll {
            site: scoop_lir::ManagedPollSite {
                target,
                safepoint: reference,
                live: scoop_lir::StatepointLiveSet::default(),
            },
        });

    assert_module_validation_error(&module, "reuses safepoint reference 702");
}

#[test]
fn safepoint_validation_rejects_unreachable_final_cfg_blocks() {
    let mut module = managed_poll_test_module();
    module.functions[0].blocks.alloc(BasicBlock {
        name: "unreachable".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return { value: None },
    });

    assert_module_validation_error(
        &module,
        "contains unreachable block 1 during safepoint validation",
    );
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
        callable_body: callable_body_at(file!(), line!()),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::Managed,
        signature: callee_signature,
        call_targets: CallTargets::default(),
        locals: Arena::new(),
        temps: Arena::new(),
        blocks: callee_blocks,
        entry: callee_entry,
    };

    let mut locals = Arena::new();
    let argument = locals.alloc(test_local("indirect_argument", aggregate.clone()));
    let mut temps = Arena::new();
    let value = temps.alloc(Temp {
        ty: aggregate.clone(),
    });
    let mut targets = CallTargets::default();
    let signature = targets.void_signatures.alloc(VoidCallSignature::new(
        vec![scoop_lir::AbiArgument::Indirect(abi_value.clone())],
        scoop_lir::CallingConvention::Cdecl,
    ));
    let storage = scoop_lir::AbiArgumentStorage::new(argument, locals[argument].ty(), &abi_value)
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
        callable_body: callable_body_at(file!(), line!()),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::Managed,
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
        1,
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
        callable_body: callable_body_at(file!(), line!()),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(vec![MANAGED_PTR], MANAGED_PTR),
        call_targets: targets,
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    };
    root_plan_test_module(vec![caller], extern_functions, 0)
}

fn managed_invoke_root_module(normal_live: bool, unwind_live: bool) -> Module {
    let mut callee_blocks = Arena::new();
    let callee_entry = callee_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return { value: None },
    });
    let callee = Function {
        callable_body: callable_body_at(file!(), line!()),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::Managed,
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
        callable_body: callable_body_at(file!(), line!()),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::Managed,
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
        1,
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

#[test]
fn root_plan_validation_rejects_omitted_managed_poll_root() {
    let mut module = managed_poll_test_module();
    let function = &mut module.functions[0];
    function.signature = plain_scoop_signature(vec![MANAGED_PTR], MANAGED_PTR);
    function.blocks[function.entry].terminator = Terminator::Return {
        value: Some(Value::Param(0)),
    };

    assert_module_validation_error(
        &module,
        "managed poll root plan in {function:0} block0 instruction 0 root plan has 0 entries, expected 1 complete entries",
    );
}

#[test]
fn scoop_abi_validation_rejects_invalid_managed_poll_target() {
    let mut module = managed_poll_test_module();
    let function = &mut module.functions[0];
    let Instruction::ManagedPoll { site } = &mut function.blocks[function.entry].instructions[0]
    else {
        panic!("managed poll fixture starts with a poll")
    };
    site.target = scoop_lir::ManagedVoidTargetId::from_raw(99.into());

    assert_module_validation_error(
        &module,
        "managed poll {function:0}: references invalid managed-void target 99",
    );
}

#[test]
fn scoop_abi_validation_rejects_non_safepoint_managed_poll_target() {
    let mut module = managed_poll_test_module();
    let function = &mut module.functions[0];
    let target = match &function.blocks[function.entry].instructions[0] {
        Instruction::ManagedPoll { site } => site.target,
        _ => panic!("managed poll fixture starts with a poll"),
    };
    function.call_targets.managed_targets.void[target].destination =
        scoop_lir::ManagedCallDestination::runtime(scoop_lir::ManagedRuntimeFunction::GcCollect);

    assert_module_validation_error(
        &module,
        "managed poll {function:0}: target is not the managed safepoint runtime function",
    );
}

#[test]
fn scoop_abi_validation_rejects_an_unknown_managed_external_call_target() {
    let mut module = managed_poll_test_module();
    let function = &mut module.functions[0];
    let signature = function
        .call_targets
        .void_signatures
        .alloc(VoidCallSignature::new(
            Vec::new(),
            scoop_lir::CallingConvention::Cdecl,
        ));
    let external = scoop_lir::ExternalCallableId::from_raw(0_u32.into());
    let target = function
        .call_targets
        .managed_targets
        .void
        .alloc(scoop_lir::CallTarget {
            destination: scoop_lir::ManagedCallDestination::external(external),
            signature,
        });
    function.blocks[function.entry]
        .instructions
        .push(Instruction::Call {
            site: scoop_lir::CallSite::Managed(scoop_lir::ManagedCallSite {
                call: scoop_lir::TypedCall::Void {
                    target,
                    args: Vec::new(),
                },
                safepoint: test_safepoint(704),
                live: scoop_lir::StatepointLiveSet::default(),
            }),
        });
    refresh_module_safepoints(&mut module);

    assert_module_validation_error(&module, "references invalid external callable 0");
}

#[test]
fn scoop_abi_validation_rejects_an_unknown_no_gc_external_call_target() {
    let mut module = managed_poll_test_module();
    let function = &mut module.functions[0];
    let external = scoop_lir::ExternalCallableId::from_raw(0_u32.into());
    let site = void_site(
        &mut function.call_targets,
        TestCallProtocol::NoGc {
            destination: scoop_lir::NoGcCallDestination::external(external),
        },
        Vec::new(),
        Vec::new(),
    );
    function.blocks[function.entry]
        .instructions
        .push(Instruction::Call { site });
    assert_module_validation_error(&module, "references invalid external callable 0");
}

#[test]
fn scoop_abi_validation_rejects_noncanonical_managed_poll_signature() {
    let mut module = managed_poll_test_module();
    let function = &mut module.functions[0];
    let target = match &function.blocks[function.entry].instructions[0] {
        Instruction::ManagedPoll { site } => site.target,
        _ => panic!("managed poll fixture starts with a poll"),
    };
    let arguments = plain_scoop_signature(vec![MANAGED_PTR], LirType::Void)
        .arguments()
        .to_vec();
    let signature = function
        .call_targets
        .void_signatures
        .alloc(VoidCallSignature::new(
            arguments,
            scoop_lir::CallingConvention::Cdecl,
        ));
    function.call_targets.managed_targets.void[target].signature = signature;

    assert_module_validation_error(
        &module,
        "managed poll {function:0}: target must use the exact `cdecl () -> void` safepoint ABI",
    );
}

#[test]
fn scoop_abi_validation_rejects_managed_poll_in_no_gc_function() {
    let mut module = managed_poll_test_module();
    module.functions[0].gc_effect = GcEffect::NoGc;
    module.output = scoop_lir::LirOutput::Executable {
        entry: no_gc_function_ref(0),
    };

    assert_module_validation_error(
        &module,
        "managed poll {function:0}: is only valid in a managed function",
    );
}
