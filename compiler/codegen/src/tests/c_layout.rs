use super::*;

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
                adapter_symbol: adapter.to_string(),
                trampoline_symbol: "scoop_foreign_callback_0".to_string(),
                signature_symbol: "scoop_foreign_callback_signature_0".to_string(),
                params: vec![scoop_lir::CType::Int, scoop_lir::CType::Pointer],
                return_type: scoop_lir::CType::Int,
                context_index: 1,
                mode,
            });
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
        ir.contains("getelementptr i8, ptr addrspace(1) %managed_object, i32 32"),
        "over-aligned array data must start at offset 32:\n{ir}"
    );
    assert!(
        ir.contains(
            "@scoop_runtime_finish_tlab_alloc(ptr addrspace(1) %tlab_object, ptr @scoop_td_ArrayOuter, i64 64)"
        ) && ir.contains("@scoop_runtime_alloc_slow(ptr @scoop_td_ArrayOuter, i64 64)"),
        "one 32-byte element plus the aligned 32-byte header must flow through the 64-byte TLAB check:\n{ir}"
    );
}
