use super::*;

fn foreign_callback_adapter(symbol: &str) -> Function {
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

fn callback_struct(module: &mut Module, name: &str) -> scoop_lir::StructDefId {
    module.structs.alloc(StructDef {
        name: name.to_string(),
        fields: vec![
            scoop_lir::StructField {
                ty: CODE_PTR,
                layout: scoop_lir::FieldLayout {
                    offset: 0,
                    access_align: 8,
                },
            },
            scoop_lir::StructField {
                ty: RAW_PTR,
                layout: scoop_lir::FieldLayout {
                    offset: 8,
                    access_align: 8,
                },
            },
        ],
        size: 16,
        align: 8,
        c_layout: None,
        interior_mutable: false,
    })
}

fn callback_state(module: &mut Module, name: &str) -> scoop_lir::EnumDefId {
    module.enums.alloc(EnumDef {
        name: name.to_string(),
        repr: EnumRepr::Tagged {
            variants: (0..4)
                .map(|_| EnumVariantRepr {
                    fields: Vec::new(),
                    slot_offset: 8,
                    slot_size: 0,
                    slot_align: 1,
                    gc_free: true,
                })
                .collect(),
            size: 8,
            align: 8,
        },
        scan: RefScan::None,
    })
}

fn callback_failure(module: &mut Module, name: &str) -> scoop_lir::EnumDefId {
    module.enums.alloc(EnumDef {
        name: name.to_string(),
        repr: EnumRepr::Niche { payload_variant: 0 },
        scan: RefScan::References(vec![0]),
    })
}

pub(super) fn foreign_callback_family(module: &mut Module) -> scoop_lir::ForeignCallbackFamilyId {
    let callback = callback_struct(module, "ForeignCallback<F>");
    let state = callback_state(module, "ForeignCallbackState");
    let failure = callback_failure(module, "Option<Throwable>");
    module
        .foreign_callback_families
        .alloc(scoop_lir::ForeignCallbackFamily {
            callback,
            state,
            failure,
        })
}

struct ForeignCallbackBridgeFixture<'a> {
    adapter: &'a str,
    trampoline: &'a str,
    signature: &'a str,
    params: Vec<scoop_lir::CType>,
    return_type: scoop_lir::CType,
    context_index: u32,
}

fn add_foreign_callback_bridge(
    module: &mut Module,
    family: scoop_lir::ForeignCallbackFamilyId,
    fixture: ForeignCallbackBridgeFixture<'_>,
) {
    module
        .foreign_callback_bridges
        .alloc(scoop_lir::ForeignCallbackBridge {
            family,
            adapter_symbol: fixture.adapter.to_string(),
            trampoline_symbol: fixture.trampoline.to_string(),
            signature_symbol: fixture.signature.to_string(),
            params: fixture.params,
            return_type: fixture.return_type,
            context_index: fixture.context_index,
            mode: scoop_lir::ForeignCallbackMode::Reusable,
        });
    module
        .functions
        .push(foreign_callback_adapter(fixture.adapter));
}

#[test]
fn c_layout_matches_llvm_and_generated_c_assertions() {
    let mut structs = Arena::default();
    let inner = structs.alloc(StructDef {
        name: "Inner".to_string(),
        fields: vec![
            scoop_lir::StructField {
                ty: LirType::I1,
                layout: scoop_lir::FieldLayout {
                    offset: 0,
                    access_align: 1,
                },
            },
            scoop_lir::StructField {
                ty: LirType::I64,
                layout: scoop_lir::FieldLayout {
                    offset: 1,
                    access_align: 1,
                },
            },
        ],
        size: 16,
        align: 8,
        c_layout: Some(scoop_lir::CLayout {
            aligned: 8,
            packed: 1,
        }),
        interior_mutable: false,
    });
    let outer = structs.alloc(StructDef {
        name: "Outer".to_string(),
        fields: vec![
            scoop_lir::StructField {
                ty: LirType::I1,
                layout: scoop_lir::FieldLayout {
                    offset: 0,
                    access_align: 1,
                },
            },
            scoop_lir::StructField {
                ty: LirType::Struct(inner),
                layout: scoop_lir::FieldLayout {
                    offset: 2,
                    access_align: 2,
                },
            },
            scoop_lir::StructField {
                ty: LirType::I64,
                layout: scoop_lir::FieldLayout {
                    offset: 18,
                    access_align: 2,
                },
            },
        ],
        size: 32,
        align: 16,
        c_layout: Some(scoop_lir::CLayout {
            aligned: 16,
            packed: 2,
        }),
        interior_mutable: true,
    });
    let mut enums = Arena::default();
    let wrapped = enums.alloc(EnumDef {
        name: "Wrapped".to_string(),
        repr: EnumRepr::Tagged {
            variants: vec![
                EnumVariantRepr {
                    fields: vec![EnumFieldRepr {
                        ty: LirType::Struct(outer),
                        offset: 16,
                    }],
                    slot_offset: 16,
                    slot_size: 32,
                    slot_align: 16,
                    gc_free: true,
                },
                EnumVariantRepr {
                    fields: Vec::new(),
                    slot_offset: 16,
                    slot_size: 0,
                    slot_align: 1,
                    gc_free: true,
                },
            ],
            size: 48,
            align: 16,
        },
        scan: RefScan::None,
    });

    let mut meta = string_metadata();
    let outer_array = array_type(
        &mut meta,
        "ArrayOuter",
        scoop_lir::ArrayKind::Immutable,
        LirType::Struct(outer),
        32,
        16,
        RefScan::None,
    );

    let mut temps = Arena::default();
    let inner_value = temps.alloc(Temp {
        ty: LirType::Struct(inner),
    });
    let inner_field = temps.alloc(Temp { ty: LirType::I64 });
    let outer_value = temps.alloc(Temp {
        ty: LirType::Struct(outer),
    });
    let outer_field = temps.alloc(Temp {
        ty: LirType::Struct(inner),
    });
    let array = temps.alloc(Temp { ty: MANAGED_PTR });
    let loaded = temps.alloc(Temp {
        ty: LirType::Struct(outer),
    });
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::MakeAggregate {
                out: inner_value,
                elements: vec![Value::BoolConst(true), Value::IntConst(7)],
            },
            Instruction::ExtractValue {
                out: inner_field,
                aggregate: Value::Temp(inner_value),
                index: 1,
            },
            Instruction::MakeAggregate {
                out: outer_value,
                elements: vec![
                    Value::BoolConst(false),
                    Value::Temp(inner_value),
                    Value::IntConst(9),
                ],
            },
            Instruction::ExtractValue {
                out: outer_field,
                aggregate: Value::Temp(outer_value),
                index: 1,
            },
            Instruction::ArrayAlloc {
                out: array,
                elements: vec![Value::Temp(outer_value)],
                array_type: outer_array,
                safepoint: test_safepoint(1),
                live: scoop_lir::StatepointLiveSet::default(),
            },
            Instruction::ArrayGet {
                out: loaded,
                array: Value::Temp(array),
                index: Value::IntConst(0),
                array_type: outer_array,
            },
        ],
        terminator: Terminator::Return { value: None },
    });
    let mut module = Module {
        globals: Arena::default(),
        initialization_units: Arena::default(),
        structs,
        enums,
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_families: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![Function {
            gc_effect: GcEffect::Managed,
            symbol: "scoop_main".to_string(),
            params: vec![],
            return_ty: LirType::Void,
            call_targets: CallTargets::default(),
            locals: Arena::default(),
            temps,
            blocks,
            entry,
        }],
        entry_symbol: "scoop_main".to_string(),
        meta,
    };

    let machine = host_target_machine().expect("target machine");
    let target_data = machine.get_target_data();
    let context = Context::create();
    let outer_ty = basic_ty(
        &context,
        &module.structs,
        &module.enums,
        host_managed_address_space(),
        &LirType::Struct(outer),
    )
    .expect("outer LLVM type");
    assert_eq!(target_data.get_abi_size(&outer_ty), 32);
    assert_eq!(target_data.get_abi_alignment(&outer_ty), 16);
    let wrapped_ty = basic_ty(
        &context,
        &module.structs,
        &module.enums,
        host_managed_address_space(),
        &LirType::Enum(wrapped),
    )
    .expect("wrapped LLVM type");
    assert_eq!(target_data.get_abi_size(&wrapped_ty), 48);
    assert_eq!(target_data.get_abi_alignment(&wrapped_ty), 16);

    let assertions = c_layout_assertions(&module).expect("C assertions");
    assert!(
        assertions.find("scoop_c_layout_0").unwrap() < assertions.find("scoop_c_layout_1").unwrap(),
        "nested declaration must precede its user:\n{assertions}"
    );
    assert!(assertions.contains("offsetof(scoop_c_layout_1, _field_1) == 2"));
    assert!(assertions.contains("_Alignof(scoop_c_layout_1) == 16"));
    let source = std::env::temp_dir().join(format!(
        "scoop_c_layout_assertions_{}.c",
        std::process::id()
    ));
    std::fs::write(&source, &assertions).expect("write generated C");
    let status = std::process::Command::new("cc")
        .args(["-std=c11", "-fsyntax-only"])
        .arg(&source)
        .status()
        .expect("run C compiler");
    std::fs::remove_file(&source).ok();
    assert!(status.success(), "generated C assertions must compile");

    module.extern_functions.alloc_c(scoop_lir::CExternFunction {
        declaration: scoop_lir::ExternFunctionDeclaration {
            source_name: "swap".to_string(),
            native_symbol: "native_swap".to_string(),
            library: "fixture".to_string(),
            calling_convention: scoop_lir::CallingConvention::Cdecl,
            params: vec![LirType::Struct(outer)],
            return_type: LirType::Struct(outer),
        },
        bridge_symbol: "scoop_c_bridge_0".to_string(),
        params: vec![scoop_lir::CType::Struct(outer)],
        return_type: scoop_lir::CType::Struct(outer),
    });
    module.callback_bridges.alloc(scoop_lir::CallbackBridge {
        source_name: "swapCallback".to_string(),
        bridge_symbol: "scoop_callback_bridge_0".to_string(),
        trampoline_symbol: "scoop_c_callback_0".to_string(),
        params: vec![scoop_lir::CType::Struct(outer)],
        return_type: scoop_lir::CType::Struct(outer),
    });
    let foreign_callback_family = foreign_callback_family(&mut module);
    for (adapter, mode) in [
        (
            "scoop_foreign_callback_adapter_0",
            scoop_lir::ForeignCallbackMode::Reusable,
        ),
        (
            "scoop_foreign_callback_adapter_1",
            scoop_lir::ForeignCallbackMode::OneShot,
        ),
    ] {
        module
            .foreign_callback_bridges
            .alloc(scoop_lir::ForeignCallbackBridge {
                family: foreign_callback_family,
                adapter_symbol: adapter.to_string(),
                trampoline_symbol: "scoop_foreign_callback_0".to_string(),
                signature_symbol: "scoop_foreign_callback_signature_0".to_string(),
                params: vec![scoop_lir::CType::Int, scoop_lir::CType::Pointer],
                return_type: scoop_lir::CType::Int,
                context_index: 1,
                mode,
            });
        module.functions.push(foreign_callback_adapter(adapter));
    }
    let bridge = c_bridge_source(&module)
        .expect("C bridge")
        .expect("C extern needs a bridge");
    assert!(bridge.contains("extern scoop_c_layout_1 native_swap(scoop_c_layout_1);"));
    assert!(bridge.contains("void scoop_c_bridge_0(void *result, const void *arg0)"));
    assert!(bridge.contains("memcpy(result, &native_result, sizeof(native_result));"));
    assert!(
        bridge.contains("extern void scoop_callback_bridge_0(void *result, const void *arg0);")
    );
    assert!(bridge.contains("scoop_c_layout_1 scoop_c_callback_0(scoop_c_layout_1 arg0)"));
    assert!(bridge.contains("scoop_callback_bridge_0(&result, &arg0);"));
    assert_eq!(
        bridge
            .matches("const unsigned char scoop_foreign_callback_signature_0 = 0;")
            .count(),
        1,
        "one signature/context shape must emit one descriptor:\n{bridge}"
    );
    assert_eq!(
        bridge
            .matches("int64_t scoop_foreign_callback_0(int64_t arg0, void * arg1)")
            .count(),
        1,
        "registrations sharing a signature/context shape must share one trampoline:\n{bridge}"
    );
    assert!(bridge.contains("int64_t result = {0};"));
    assert!(bridge.contains("const void *arguments[1] = {&arg0};"));
    assert!(bridge.contains(
            "scoop_runtime_callback_invoke(arg1, &scoop_foreign_callback_signature_0, &result, arguments)"
        ));
    let bridge_source = std::env::temp_dir().join(format!(
        "scoop_c_foreign_callback_bridge_{}.c",
        std::process::id()
    ));
    std::fs::write(&bridge_source, &bridge).expect("write generated callback C");
    let status = std::process::Command::new("cc")
        .args(["-std=c11", "-fsyntax-only"])
        .arg(&bridge_source)
        .status()
        .expect("run C compiler");
    std::fs::remove_file(&bridge_source).ok();
    assert!(status.success(), "generated callback C must compile");

    let ir = ir_of(&module);
    assert!(
        ir.contains("getelementptr i8, ptr addrspace(1) %managed_object, i64 32"),
        "over-aligned array data must start at offset 32:\n{ir}"
    );
    assert!(
        ir.contains(
            "@scoop_runtime_finish_tlab_alloc(ptr addrspace(1) %tlab_object, ptr @scoop_td_ArrayOuter, i64 64)"
        ) && ir.contains("@scoop_runtime_alloc_slow(ptr @scoop_td_ArrayOuter, i64 64)"),
        "one 32-byte element plus the aligned 32-byte header must flow through the 64-byte TLAB check:\n{ir}"
    );
}

#[test]
fn c_extern_rejects_machine_scalar_lir_type() {
    let mut module = values_module();
    module.extern_functions.alloc_c(scoop_lir::CExternFunction {
        declaration: scoop_lir::ExternFunctionDeclaration {
            source_name: "machineSize".to_string(),
            native_symbol: "machine_size".to_string(),
            library: "fixture".to_string(),
            calling_convention: scoop_lir::CallingConvention::Cdecl,
            params: vec![LirType::MachineScalar(MachineScalarKind::ByteSize)],
            return_type: LirType::Void,
        },
        bridge_symbol: "scoop_c_bridge_machine_size".to_string(),
        params: vec![scoop_lir::CType::UInt],
        return_type: scoop_lir::CType::Unit,
    });

    let error = c_bridge_source(&module).expect_err("C ABI must reject internal machine scalars");
    assert!(
        error.0.contains("C extern `machineSize` parameter 0")
            && error.0.contains("machine<byte-size>"),
        "unexpected error: {error}"
    );
}

#[test]
fn native_global_rejects_machine_scalar_lir_type() {
    let mut module = values_module();
    let get = module
        .native_global_bridges
        .gets
        .alloc(scoop_lir::NativeGlobalGetBridge {
            symbol: "get_machine_global".to_string(),
        });
    let address =
        module
            .native_global_bridges
            .addresses
            .alloc(scoop_lir::NativeGlobalAddressBridge {
                symbol: "address_machine_global".to_string(),
            });
    module.native_globals.alloc(scoop_lir::NativeGlobal {
        source_name: "machineGlobal".to_string(),
        native_symbol: "machine_global".to_string(),
        library: "fixture".to_string(),
        ty: LirType::MachineScalar(MachineScalarKind::ByteSize),
        c_type: scoop_lir::CType::UInt,
        thread_local: false,
        access: scoop_lir::NativeGlobalAccess::ReadOnly { get, address },
    });

    let error =
        c_bridge_source(&module).expect_err("native C globals must reject machine scalar storage");
    assert!(
        error.0.contains("native global `machineGlobal`") && error.0.contains("machine<byte-size>"),
        "unexpected error: {error}"
    );
}

#[test]
fn foreign_callback_bridge_rejects_wrong_adapter_signature() {
    let mut module = values_module();
    let family = foreign_callback_family(&mut module);
    module
        .foreign_callback_bridges
        .alloc(scoop_lir::ForeignCallbackBridge {
            family,
            adapter_symbol: "scoop_main".to_string(),
            trampoline_symbol: "foreign_callback".to_string(),
            signature_symbol: "foreign_callback_signature".to_string(),
            params: vec![scoop_lir::CType::Pointer],
            return_type: scoop_lir::CType::Unit,
            context_index: 0,
            mode: scoop_lir::ForeignCallbackMode::Reusable,
        });

    let error = c_bridge_source(&module)
        .expect_err("foreign callback bridge must point at an exact managed adapter");
    assert!(
        error.0.contains("foreign callback adapter @scoop_main")
            && error.0.contains("machine<foreign-callback-status>"),
        "unexpected error: {error}"
    );
}

#[test]
fn foreign_callback_operation_rejects_lookalike_callback_struct() {
    let mut module = values_module();
    let family = foreign_callback_family(&mut module);
    let lookalike = callback_struct(&mut module, "LookalikeForeignCallback");
    let function = &mut module.functions[0];
    let callback = function.temps.alloc(Temp {
        ty: LirType::Struct(lookalike),
    });
    function.blocks[function.entry]
        .instructions
        .push(Instruction::ForeignCallbackOperation(
            scoop_lir::ForeignCallbackOperation::Release {
                family,
                callback: Value::Temp(callback),
            },
        ));

    let error = c_bridge_source(&module)
        .expect_err("callback operations must preserve nominal callback identity");
    assert!(
        error.0.contains("requires exact callback struct")
            && error.0.contains(&format!("struct{}", lookalike.into_raw())),
        "unexpected error: {error}"
    );
}

#[test]
fn foreign_callback_state_operation_rejects_lookalike_state_enum() {
    let mut module = values_module();
    let family = foreign_callback_family(&mut module);
    let callback = module.foreign_callback_families[family].callback;
    let lookalike = callback_state(&mut module, "LookalikeForeignCallbackState");
    let function = &mut module.functions[0];
    let callback = function.temps.alloc(Temp {
        ty: LirType::Struct(callback),
    });
    let out = function.temps.alloc(Temp {
        ty: LirType::Enum(lookalike),
    });
    function.blocks[function.entry]
        .instructions
        .push(Instruction::ForeignCallbackOperation(
            scoop_lir::ForeignCallbackOperation::State {
                family,
                out,
                callback: Value::Temp(callback),
            },
        ));

    let error = c_bridge_source(&module)
        .expect_err("callback state operations must preserve nominal state identity");
    assert!(
        error.0.contains("foreign callback operation")
            && error.0.contains("non-protocol result type"),
        "unexpected error: {error}"
    );
}

#[test]
fn foreign_callback_failure_operation_rejects_lookalike_failure_enum() {
    let mut module = values_module();
    let family = foreign_callback_family(&mut module);
    let callback = module.foreign_callback_families[family].callback;
    let lookalike = callback_failure(&mut module, "LookalikeOptionThrowable");
    let function = &mut module.functions[0];
    let callback = function.temps.alloc(Temp {
        ty: LirType::Struct(callback),
    });
    let out = function.temps.alloc(Temp {
        ty: LirType::Enum(lookalike),
    });
    function.blocks[function.entry]
        .instructions
        .push(Instruction::ForeignCallbackOperation(
            scoop_lir::ForeignCallbackOperation::Failure {
                family,
                out,
                callback: Value::Temp(callback),
            },
        ));

    let error = c_bridge_source(&module)
        .expect_err("callback failure operations must preserve nominal failure identity");
    assert!(
        error.0.contains("foreign callback operation")
            && error.0.contains("non-protocol result type"),
        "unexpected error: {error}"
    );
}

#[test]
fn foreign_callback_bridge_rejects_out_of_bounds_context_index() {
    let mut module = values_module();
    let family = foreign_callback_family(&mut module);
    add_foreign_callback_bridge(
        &mut module,
        family,
        ForeignCallbackBridgeFixture {
            adapter: "foreign_callback_adapter",
            trampoline: "foreign_callback",
            signature: "foreign_callback_signature",
            params: vec![scoop_lir::CType::Pointer],
            return_type: scoop_lir::CType::Unit,
            context_index: 1,
        },
    );

    let error = c_bridge_source(&module).expect_err("context index must name a C parameter");
    assert!(
        error
            .0
            .contains("context index 1 is outside its 1 C parameters"),
        "unexpected error: {error}"
    );
}

#[test]
fn foreign_callback_bridge_rejects_non_pointer_context() {
    let mut module = values_module();
    let family = foreign_callback_family(&mut module);
    add_foreign_callback_bridge(
        &mut module,
        family,
        ForeignCallbackBridgeFixture {
            adapter: "foreign_callback_adapter",
            trampoline: "foreign_callback",
            signature: "foreign_callback_signature",
            params: vec![scoop_lir::CType::Int],
            return_type: scoop_lir::CType::Unit,
            context_index: 0,
        },
    );

    let error = c_bridge_source(&module).expect_err("callback context must be a C pointer");
    assert!(
        error
            .0
            .contains("context parameter 0 must be CType::Pointer, found Int"),
        "unexpected error: {error}"
    );
}

#[test]
fn foreign_callback_bridge_rejects_conflicting_trampoline_abi_metadata() {
    let mut module = values_module();
    let family = foreign_callback_family(&mut module);
    add_foreign_callback_bridge(
        &mut module,
        family,
        ForeignCallbackBridgeFixture {
            adapter: "foreign_callback_adapter_0",
            trampoline: "shared_foreign_callback",
            signature: "foreign_callback_signature_0",
            params: vec![scoop_lir::CType::Pointer],
            return_type: scoop_lir::CType::Unit,
            context_index: 0,
        },
    );
    add_foreign_callback_bridge(
        &mut module,
        family,
        ForeignCallbackBridgeFixture {
            adapter: "foreign_callback_adapter_1",
            trampoline: "shared_foreign_callback",
            signature: "foreign_callback_signature_1",
            params: vec![scoop_lir::CType::Int, scoop_lir::CType::Pointer],
            return_type: scoop_lir::CType::Int,
            context_index: 1,
        },
    );

    let error = c_bridge_source(&module)
        .expect_err("a shared trampoline symbol must have one ABI description");
    assert!(
        error
            .0
            .contains("trampoline symbol @shared_foreign_callback has conflicting ABI metadata"),
        "unexpected error: {error}"
    );
}

#[test]
fn foreign_callback_bridge_rejects_conflicting_signature_abi_metadata() {
    let mut module = values_module();
    let family = foreign_callback_family(&mut module);
    add_foreign_callback_bridge(
        &mut module,
        family,
        ForeignCallbackBridgeFixture {
            adapter: "foreign_callback_adapter_0",
            trampoline: "foreign_callback_0",
            signature: "shared_foreign_callback_signature",
            params: vec![scoop_lir::CType::Pointer],
            return_type: scoop_lir::CType::Unit,
            context_index: 0,
        },
    );
    add_foreign_callback_bridge(
        &mut module,
        family,
        ForeignCallbackBridgeFixture {
            adapter: "foreign_callback_adapter_1",
            trampoline: "foreign_callback_1",
            signature: "shared_foreign_callback_signature",
            params: vec![scoop_lir::CType::Int, scoop_lir::CType::Pointer],
            return_type: scoop_lir::CType::Int,
            context_index: 1,
        },
    );

    let error = c_bridge_source(&module)
        .expect_err("a shared signature symbol must have one ABI description");
    assert!(
        error.0.contains(
            "signature symbol @shared_foreign_callback_signature has conflicting ABI metadata"
        ),
        "unexpected error: {error}"
    );
}
