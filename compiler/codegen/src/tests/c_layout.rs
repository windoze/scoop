use super::*;

pub(super) fn foreign_callback_adapter(symbol: &str) -> Function {
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

fn static_callback_bridge(module: &mut Module, symbol: &str) -> scoop_lir::NoGcLocalFunctionRef {
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return { value: None },
    });
    let index = module.functions.len();
    module.functions.push(Function {
        callable_body: callable_body(symbol),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::NoGc,
        signature: plain_scoop_signature(Vec::new(), LirType::Void),
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps: Arena::default(),
        blocks,
        entry,
    });
    no_gc_local_function_ref(index)
}

fn callback_struct(module: &mut Module, name: &str) -> scoop_lir::StructDefId {
    module.structs.alloc_scoop(
        crate::tests::test_physical_exact(name, scoop_identity::SourceNominalKind::Struct),
        name.to_string(),
        16,
        8,
        false,
        vec![
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
    )
}

fn callback_state(module: &mut Module, name: &str) -> scoop_lir::EnumDefId {
    module.enums.alloc(EnumDef {
        exact_type: crate::tests::test_physical_exact(
            name,
            scoop_identity::SourceNominalKind::Enum,
        ),
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

fn callback_mode(module: &mut Module) -> scoop_lir::EnumDefId {
    module.enums.alloc(EnumDef {
        exact_type: crate::tests::test_physical_exact(
            "ForeignCallbackMode",
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: "ForeignCallbackMode".to_string(),
        repr: EnumRepr::Tagged {
            variants: (0..2)
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
        exact_type: crate::tests::test_physical_exact(
            name,
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: name.to_string(),
        repr: EnumRepr::Niche {
            kind: scoop_lir::NichePointerKind::Managed,
            payload_variant: 0,
        },
        scan: RefScan::References(vec![0]),
    })
}

fn pointer_niche(
    module: &mut Module,
    name: &str,
    kind: scoop_lir::NichePointerKind,
) -> scoop_lir::EnumDefId {
    let definition = EnumDef {
        exact_type: crate::tests::test_physical_exact(
            name,
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: name.to_string(),
        repr: EnumRepr::Niche {
            kind,
            payload_variant: 0,
        },
        scan: if kind == scoop_lir::NichePointerKind::Managed {
            RefScan::References(vec![0])
        } else {
            RefScan::None
        },
    };
    match kind {
        scoop_lir::NichePointerKind::Raw => module
            .enums
            .alloc_c_nullable_data_pointer_option(definition),
        scoop_lir::NichePointerKind::Code => module
            .enums
            .alloc_c_nullable_code_pointer_option(definition),
        scoop_lir::NichePointerKind::Managed => module.enums.alloc(definition),
    }
}

fn c_nullable_function_pointer(
    enums: &scoop_lir::EnumDefs,
    enum_id: scoop_lir::EnumDefId,
) -> scoop_lir::CType {
    let signature = scoop_lir::CFunctionType {
        params: Vec::new(),
        return_type: scoop_lir::CReturnType::Void,
    };
    scoop_lir::CType::CodePointer {
        signature: Box::new(signature.clone()),
        storage: scoop_lir::CCodePointerStorage::Nullable(
            enums
                .nullable_code_pointer_ref(enum_id, signature)
                .expect("test code-pointer niche"),
        ),
    }
}

pub(super) fn c_opaque_pointer() -> scoop_lir::CType {
    scoop_lir::CType::DataPointer {
        pointee: scoop_lir::CDataPointee::OpaqueVoid,
        storage: scoop_lir::CDataPointerStorage::Direct,
    }
}

fn c_nullable_opaque_pointer(
    enums: &scoop_lir::EnumDefs,
    enum_id: scoop_lir::EnumDefId,
) -> scoop_lir::CType {
    let pointee = scoop_lir::CDataPointee::OpaqueVoid;
    scoop_lir::CType::DataPointer {
        pointee: pointee.clone(),
        storage: scoop_lir::CDataPointerStorage::Nullable(
            enums
                .nullable_data_pointer_ref(enum_id, pointee)
                .expect("test raw-pointer niche"),
        ),
    }
}

fn c_struct(structs: &scoop_lir::StructDefs, id: scoop_lir::StructDefId) -> scoop_lir::CType {
    scoop_lir::CType::Struct(structs.c_ref(id).expect("test C struct"))
}

pub(super) fn c_value(ty: scoop_lir::CType) -> scoop_lir::CReturnType {
    scoop_lir::CReturnType::Value(Box::new(ty))
}

pub(super) fn foreign_callback_family(module: &mut Module) -> scoop_lir::ForeignCallbackFamilyId {
    let callback = callback_struct(module, "ForeignCallback<F>");
    let mode = callback_mode(module);
    let state = callback_state(module, "ForeignCallbackState");
    let failure = callback_failure(module, "Option<Throwable>");
    let modes = scoop_lir::ForeignCallbackModes::checked(
        &module.enums,
        module.enums.variant_ref(mode, 0).expect("Reusable"),
        module.enums.variant_ref(mode, 1).expect("OneShot"),
    )
    .expect("callback modes");
    let states = scoop_lir::ForeignCallbackStates::checked(
        &module.enums,
        module.enums.variant_ref(state, 0).expect("Registered"),
        module.enums.variant_ref(state, 1).expect("Active"),
        module.enums.variant_ref(state, 2).expect("Completed"),
        module.enums.variant_ref(state, 3).expect("Failed"),
    )
    .expect("callback states");
    let some = module.enums.variant_ref(failure, 0).expect("Some");
    let failure_result = scoop_lir::ForeignCallbackFailureResult::checked(
        &module.enums,
        module
            .enums
            .variant_field_ref(some, 0)
            .expect("Some payload"),
        module.enums.variant_ref(failure, 1).expect("None"),
    )
    .expect("callback failure result");
    module
        .foreign_callback_families
        .alloc(scoop_lir::ForeignCallbackFamily {
            callback,
            modes,
            states,
            failure_result,
        })
}

struct ForeignCallbackBridgeFixture<'a> {
    adapter: &'a str,
    identity_seed: u8,
    params: Vec<scoop_lir::CType>,
    return_type: scoop_lir::CReturnType,
    context_index: u32,
}

fn add_foreign_callback_bridge(
    module: &mut Module,
    family: scoop_lir::ForeignCallbackFamilyId,
    fixture: ForeignCallbackBridgeFixture<'_>,
) {
    install_test_c_signature(module, fixture.identity_seed);
    let ordinal =
        u32::try_from(module.foreign_callback_bridges.len()).expect("bridge count fits u32");
    let application = callback_application(ordinal);
    let adapter_index = module.functions.len();
    module
        .functions
        .push(foreign_callback_adapter(fixture.adapter));
    module
        .foreign_callback_bridges
        .alloc(scoop_lir::ForeignCallbackBridge {
            application,
            family,
            adapter: managed_local_function_ref(adapter_index),
            trampoline: callback_trampoline(fixture.identity_seed, fixture.context_index),
            params: fixture.params,
            return_type: fixture.return_type,
            context_index: fixture.context_index,
            mode: module.foreign_callback_families[family].modes.reusable(),
        });
}

#[test]
fn c_layout_matches_llvm_and_generated_c_assertions() {
    let mut structs = scoop_lir::StructDefs::default();
    let inner = structs.alloc_c(
        crate::tests::test_physical_exact("Inner", scoop_identity::SourceNominalKind::Struct),
        "Inner".to_string(),
        16,
        8,
        false,
        scoop_lir::LirCLayoutContract {
            aligned: scoop_lir::LirCLayoutValue::A8,
            packed: scoop_lir::LirCLayoutValue::A1,
        },
        vec![
            scoop_lir::CStructField {
                identity: test_field_identity("Inner", "flag"),
                ty: scoop_lir::CType::Boolean,
                layout: scoop_lir::FieldLayout {
                    offset: 0,
                    access_align: 1,
                },
            },
            scoop_lir::CStructField {
                identity: test_field_identity("Inner", "value"),
                ty: scoop_lir::CType::Integer(IntegerKind::SIGNED_64),
                layout: scoop_lir::FieldLayout {
                    offset: 1,
                    access_align: 1,
                },
            },
        ],
    );
    let inner_id = inner.definition();
    let outer = structs.alloc_c(
        crate::tests::test_physical_exact("Outer", scoop_identity::SourceNominalKind::Struct),
        "Outer".to_string(),
        32,
        16,
        true,
        scoop_lir::LirCLayoutContract {
            aligned: scoop_lir::LirCLayoutValue::A16,
            packed: scoop_lir::LirCLayoutValue::A2,
        },
        vec![
            scoop_lir::CStructField {
                identity: test_field_identity("Outer", "flag"),
                ty: scoop_lir::CType::Boolean,
                layout: scoop_lir::FieldLayout {
                    offset: 0,
                    access_align: 1,
                },
            },
            scoop_lir::CStructField {
                identity: test_field_identity("Outer", "inner"),
                ty: scoop_lir::CType::Struct(inner),
                layout: scoop_lir::FieldLayout {
                    offset: 2,
                    access_align: 2,
                },
            },
            scoop_lir::CStructField {
                identity: test_field_identity("Outer", "value"),
                ty: scoop_lir::CType::Integer(IntegerKind::SIGNED_64),
                layout: scoop_lir::FieldLayout {
                    offset: 18,
                    access_align: 2,
                },
            },
        ],
    );
    let outer_id = outer.definition();
    let mut enums = scoop_lir::EnumDefs::default();
    let wrapped = enums.alloc(EnumDef {
        exact_type: crate::tests::test_physical_exact(
            "Wrapped",
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: "Wrapped".to_string(),
        repr: EnumRepr::Tagged {
            variants: vec![
                EnumVariantRepr {
                    fields: vec![EnumFieldRepr {
                        ty: LirType::Struct(outer_id),
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
    let bool_exact = test_exact_type("Bool");
    let int64_exact = test_exact_type("Int64");
    let inner_exact = test_exact_type("Inner");
    let inner_layout = scoop_identity::CanonicalCAbiLayoutFingerprintRecord::new(
        scoop_identity::CanonicalCAbiLayout::new(
            inner_exact,
            16,
            std::num::NonZeroU64::new(8).unwrap(),
            scoop_identity::CLayoutOverride::Bytes(scoop_identity::CLayoutByteAlignment::Bytes8),
            scoop_identity::CLayoutOverride::Bytes(scoop_identity::CLayoutByteAlignment::Bytes1),
            vec![
                scoop_identity::CanonicalCAbiLayoutField::new(
                    test_field_identity("Inner", "flag"),
                    0,
                    scoop_identity::CanonicalCStorageType::Boolean {
                        exact_type: bool_exact,
                    },
                ),
                scoop_identity::CanonicalCAbiLayoutField::new(
                    test_field_identity("Inner", "value"),
                    1,
                    scoop_identity::CanonicalCStorageType::Integer {
                        exact_type: int64_exact,
                        signedness: scoop_identity::Signedness::Signed,
                        bit_width: scoop_identity::IntegerBitWidth::Bits64,
                    },
                ),
            ],
        ),
    )
    .unwrap();
    let outer_exact = test_exact_type("Outer");
    let outer_layout = scoop_identity::CanonicalCAbiLayoutFingerprintRecord::new(
        scoop_identity::CanonicalCAbiLayout::new(
            outer_exact,
            32,
            std::num::NonZeroU64::new(16).unwrap(),
            scoop_identity::CLayoutOverride::Bytes(scoop_identity::CLayoutByteAlignment::Bytes16),
            scoop_identity::CLayoutOverride::Bytes(scoop_identity::CLayoutByteAlignment::Bytes2),
            vec![
                scoop_identity::CanonicalCAbiLayoutField::new(
                    test_field_identity("Outer", "flag"),
                    0,
                    scoop_identity::CanonicalCStorageType::Boolean {
                        exact_type: bool_exact,
                    },
                ),
                scoop_identity::CanonicalCAbiLayoutField::new(
                    test_field_identity("Outer", "inner"),
                    2,
                    scoop_identity::CanonicalCStorageType::Struct {
                        exact_type: inner_exact,
                        layout: inner_layout.fingerprint(),
                    },
                ),
                scoop_identity::CanonicalCAbiLayoutField::new(
                    test_field_identity("Outer", "value"),
                    18,
                    scoop_identity::CanonicalCStorageType::Integer {
                        exact_type: int64_exact,
                        signedness: scoop_identity::Signedness::Signed,
                        bit_width: scoop_identity::IntegerBitWidth::Bits64,
                    },
                ),
            ],
        ),
    )
    .unwrap();
    let outer_storage = scoop_identity::CanonicalCStorageType::Struct {
        exact_type: outer_exact,
        layout: outer_layout.fingerprint(),
    };
    let outer_signature = scoop_identity::CanonicalCAbiSignatureFingerprintRecord::new(
        scoop_identity::CanonicalCAbiFunctionSignature::cdecl(
            vec![scoop_identity::CanonicalCAbiParameter::new(outer_exact, outer_storage).unwrap()],
            scoop_identity::CanonicalCAbiReturn::value(outer_exact, outer_storage).unwrap(),
        ),
    )
    .unwrap();
    meta.canonical_c_abi =
        scoop_lir::CanonicalCAbiMetadata::checked(Vec::new(), vec![inner_layout, outer_layout])
            .unwrap();
    let outer_array = array_type(
        &mut meta,
        "ArrayOuter",
        scoop_lir::ArrayKind::Immutable,
        LirType::Struct(outer_id),
        32,
        16,
        RefScan::None,
    );

    let mut temps = Arena::default();
    let inner_value = temps.alloc(Temp {
        ty: LirType::Struct(inner_id),
    });
    let inner_field = temps.alloc(Temp { ty: LirType::I64 });
    let outer_value = temps.alloc(Temp {
        ty: LirType::Struct(outer_id),
    });
    let outer_field = temps.alloc(Temp {
        ty: LirType::Struct(inner_id),
    });
    let array = temps.alloc(Temp { ty: MANAGED_PTR });
    let loaded = temps.alloc(Temp {
        ty: LirType::Struct(outer_id),
    });
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::MakeAggregate {
                out: inner_value,
                elements: vec![Value::BoolConst(true), signed64(7)],
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
                    signed64(9),
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
                index: signed64(0),
                array_type: outer_array,
            },
        ],
        terminator: Terminator::Return { value: None },
    });
    let mut module = Module {
        release_hooks: Default::default(),
        cone: scoop_identity::ConeIdentity::SINGLE_FILE,
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
            callable_body: callable_body_at(file!(), line!()),
            safepoints: scoop_lir::SafepointIdentities::default(),
            gc_effect: GcEffect::Managed,
            signature: plain_scoop_signature(vec![], LirType::Void),
            call_targets: CallTargets::default(),
            locals: Arena::default(),
            temps,
            blocks,
            entry,
        }],
        output: scoop_lir::LirOutput::Executable {
            entry: managed_function_ref(0),
        },
        meta,
    };
    refresh_module_safepoints(&mut module);

    let machine = host_target_machine().expect("target machine");
    let target_data = machine.get_target_data();
    let context = Context::create();
    let outer_ty = basic_ty(
        &context,
        &module.structs,
        &module.enums,
        host_managed_address_space(),
        &LirType::Struct(outer_id),
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
    assert!(assertions.contains("_Static_assert(CHAR_BIT == 8"));
    assert!(assertions.contains("_Static_assert(UINTPTR_MAX == UINT64_MAX"));
    assert!(assertions.contains("_Static_assert(sizeof(void *) == 8"));
    assert!(assertions.contains("_Static_assert(_Alignof(void *) == 8"));
    assert!(assertions.contains("sizeof(scoop_target_function_pointer) == 8"));
    assert!(assertions.contains("_Alignof(scoop_target_function_pointer) == 8"));
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

    install_test_native_function_contract_with_signature(&mut module, 1, &outer_signature);
    let outbound = outbound_bridge_with_signature(1, &outer_signature);
    module.extern_functions.alloc_c(scoop_lir::CExternFunction {
        call_mode: scoop_identity::CAbiCallMode::NativeSafe,
        identity: scoop_lir::ExternFunctionIdentity {
            source_name: "swap".to_string(),
            native_symbol: "native_swap".to_string(),
            library: "fixture".to_string(),
            calling_convention: scoop_lir::CallingConvention::Cdecl,
        },
        bridge: outbound,
        signature: scoop_lir::CFunctionType {
            params: vec![scoop_lir::CType::Struct(outer)],
            return_type: c_value(scoop_lir::CType::Struct(outer)),
        },
    });
    let static_bridge = static_callback_bridge(&mut module, "scoop_callback_bridge_0");
    install_test_c_signature_record(&mut module, outer_signature.clone());
    let static_trampoline =
        static_callback_trampoline_with_signature(3, outer_signature.fingerprint());
    module.callback_bridges.alloc(scoop_lir::CallbackBridge {
        source_name: "swapCallback".to_string(),
        bridge: scoop_lir::StaticCallbackTarget::Local(static_bridge),
        trampoline: static_trampoline,
        params: vec![scoop_lir::CType::Struct(outer)],
        return_type: c_value(scoop_lir::CType::Struct(outer)),
    });
    let foreign_callback_family = foreign_callback_family(&mut module);
    let callback_modes = module.foreign_callback_families[foreign_callback_family].modes;
    for (adapter, mode) in [
        (
            "scoop_foreign_callback_adapter_0",
            callback_modes.reusable(),
        ),
        (
            "scoop_foreign_callback_adapter_1",
            callback_modes.one_shot(),
        ),
    ] {
        install_test_c_signature(&mut module, 2);
        let application = callback_application(
            u32::try_from(module.foreign_callback_bridges.len()).expect("bridge count fits u32"),
        );
        let adapter_index = module.functions.len();
        module.functions.push(foreign_callback_adapter(adapter));
        module
            .foreign_callback_bridges
            .alloc(scoop_lir::ForeignCallbackBridge {
                application,
                family: foreign_callback_family,
                adapter: managed_local_function_ref(adapter_index),
                trampoline: callback_trampoline(2, 1),
                params: vec![
                    scoop_lir::CType::Integer(IntegerKind::SIGNED_64),
                    c_opaque_pointer(),
                ],
                return_type: c_value(scoop_lir::CType::Integer(IntegerKind::SIGNED_64)),
                context_index: 1,
                mode,
            });
    }
    let foundation = scoop_lir::CanonicalLirFoundation::from_module(&module)
        .expect("callback bridge identities project into the LIR foundation");
    let counts = foundation.counts();
    assert_eq!(counts.bridge_units, 3);
    assert_eq!(counts.bridge_atoms, 8);
    assert_eq!(counts.callback_bridges, 2);
    let outbound_symbol = match &module
        .extern_functions
        .iter()
        .next()
        .expect("C extern")
        .1
        .kind
    {
        scoop_lir::ExternFunctionKind::C { bridge, .. } => bridge.symbol().to_string(),
        scoop_lir::ExternFunctionKind::Scoop { .. } => panic!("expected C extern"),
    };
    let foreign_trampoline = module
        .foreign_callback_bridges
        .iter()
        .next()
        .expect("foreign callback bridge")
        .1
        .trampoline
        .clone();
    let bridge_sources = crate::c_bridge::render_c_bridge_source_set_for_module(&module)
        .expect("C extern needs bridge sources");
    assert_eq!(bridge_sources.units().len(), 3);
    assert!(
        bridge_sources
            .units()
            .windows(2)
            .all(|units| units[0].unit() < units[1].unit()),
        "generated C sources must follow canonical unit order"
    );
    let bridge = bridge_sources
        .units()
        .iter()
        .map(|unit| unit.source())
        .collect::<Vec<_>>()
        .join("\n");
    let static_callback = module
        .callback_bridges
        .iter()
        .next()
        .expect("callback bridge")
        .1;
    let scoop_lir::StaticCallbackTarget::Local(bridge_function) = static_callback.bridge else {
        panic!("local callback bridge");
    };
    let callback_bridge_symbol =
        module.functions[bridge_function.declaration().into_u32() as usize].symbol();
    let static_trampoline_symbol = static_callback.trampoline.entry().symbol();
    let callback_bridge_object_symbol = module
        .meta
        .target_profile
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(callback_bridge_symbol);
    assert_eq!(
        bridge_sources.plan().units().len(),
        bridge_sources.units().len()
    );
    let primary_symbols = [
        outbound_symbol.as_str(),
        static_trampoline_symbol,
        foreign_trampoline.entry().symbol(),
    ];
    for (index, symbol) in primary_symbols.iter().enumerate() {
        let owners = bridge_sources
            .units()
            .iter()
            .filter(|unit| unit.source().contains(symbol))
            .count();
        assert_eq!(owners, 1, "primary {symbol} must belong to one C source");
        let owner = bridge_sources
            .units()
            .iter()
            .find(|unit| unit.source().contains(symbol))
            .unwrap();
        for unrelated in primary_symbols.iter().skip(index + 1) {
            assert!(
                !owner.source().contains(unrelated),
                "unit {} leaked unrelated primary {unrelated}",
                owner.unit()
            );
        }
    }
    assert!(bridge.contains("extern scoop_c_layout_1 native_swap(scoop_c_layout_1);"));
    assert!(bridge.contains(&format!(
        "void {outbound_symbol}(void *result, const void *arg0)"
    )));
    assert!(bridge.contains("__builtin_memcpy(result, &native_result, sizeof(native_result));"));
    assert!(!bridge.contains("#include <string.h>"));
    assert!(bridge.contains(&format!(
        "extern void scoop_callback_storage_bridge(void *result, const void *arg0) __asm__(\"{callback_bridge_object_symbol}\");"
    )));
    assert!(bridge.contains(&format!(
        "scoop_c_layout_1 {static_trampoline_symbol}(scoop_c_layout_1 arg0)"
    )));
    assert!(bridge.contains("scoop_callback_storage_bridge(&result, &arg0);"));
    assert_eq!(
        bridge
            .matches(&format!(
                "const unsigned char {} __attribute__((section(\"__TEXT,__scoop_sig\"))) = 0;",
                foreign_trampoline.signature_descriptor_symbol()
            ))
            .count(),
        1,
        "one signature/context shape must emit one descriptor:\n{bridge}"
    );
    assert_eq!(
        bridge
            .matches(&format!(
                "int64_t {}(int64_t arg0, void *arg1)",
                foreign_trampoline.entry().symbol()
            ))
            .count(),
        1,
        "registrations sharing a signature/context shape must share one trampoline:\n{bridge}"
    );
    assert!(bridge.contains("int64_t result = {0};"));
    assert!(bridge.contains("const void *arguments[1] = {&arg0};"));
    assert!(bridge.contains(&format!(
        "scoop_runtime_callback_invoke(arg1, &{}, &result, arguments)",
        foreign_trampoline.signature_descriptor_symbol()
    )));
    for unit in bridge_sources.units() {
        let bridge_source = std::env::temp_dir().join(format!(
            "scoop_c_bridge_{}_{}.c",
            std::process::id(),
            unit.unit()
        ));
        std::fs::write(&bridge_source, unit.source()).expect("write generated callback C");
        let status = std::process::Command::new("cc")
            .args(["-std=c11", "-fsyntax-only"])
            .arg(&bridge_source)
            .status()
            .expect("run C compiler");
        std::fs::remove_file(&bridge_source).ok();
        assert!(status.success(), "generated callback C must compile");
    }

    let array_outer_symbol = type_descriptor_symbol(&module, "ArrayOuter");
    let ir = ir_of(&module);
    assert!(
        ir.contains("getelementptr i8, ptr addrspace(1) %managed_object, i64 32"),
        "over-aligned array data must start at offset 32:\n{ir}"
    );
    assert!(
        ir.contains(&format!(
            "@scoop_runtime_finish_tlab_alloc(ptr addrspace(1) %tlab_object, ptr @\"{array_outer_symbol}\", i64 %alloc_size)"
        )) && ir.contains(&format!(
            "@scoop_runtime_alloc_slow(ptr @\"{array_outer_symbol}\", i64 64)"
        )) && ir.contains("%alloc_size_plus_align = add i64 64, %allocation_alignment_mask")
            && ir.contains("%alloc_is_regular = icmp ule i64 %alloc_size, 32640"),
        "one 32-byte element plus the aligned 32-byte header must flow through the regular TLAB check:\n{ir}"
    );
}

#[test]
fn generated_c_source_set_is_empty_when_the_bridge_plan_is_empty() {
    let module = values_module();
    let sources = crate::c_bridge::render_c_bridge_source_set_for_module(&module)
        .expect("bridge-free module has a canonical empty source set");

    assert!(sources.plan().units().is_empty());
    assert!(sources.units().is_empty());
}

#[test]
fn c_extern_derives_physical_signature_from_exact_c_types() {
    let mut module = values_module();
    install_test_native_function_contract(&mut module, 3);
    let function = module.extern_functions.alloc_c(scoop_lir::CExternFunction {
        call_mode: scoop_identity::CAbiCallMode::NativeSafe,
        identity: scoop_lir::ExternFunctionIdentity {
            source_name: "machineSize".to_string(),
            native_symbol: "machine_size".to_string(),
            library: "fixture".to_string(),
            calling_convention: scoop_lir::CallingConvention::Cdecl,
        },
        bridge: outbound_bridge(3),
        signature: scoop_lir::CFunctionType {
            params: vec![scoop_lir::CType::Integer(IntegerKind::UNSIGNED_64)],
            return_type: scoop_lir::CReturnType::Void,
        },
    });

    let scoop_lir::ExternFunctionKind::C { signature, .. } =
        &module.extern_functions[function.declaration()].kind
    else {
        panic!("test allocated a C extern")
    };
    assert_eq!(signature.storage_params(), vec![LirType::I64]);
    assert_eq!(signature.storage_return_type(), LirType::Void);
    let bridge = joined_c_bridge_sources(&module).expect("C extern emits a bridge source");
    assert!(bridge.contains("extern void machine_size(uint64_t);"));
}

fn append_c_void_call(
    module: &mut Module,
    parameter: scoop_lir::CType,
    storage_type: LirType,
    argument: impl FnOnce(scoop_lir::LocalId) -> Value,
) {
    install_test_native_function_contract(module, 4);
    let function = module.extern_functions.alloc_c(scoop_lir::CExternFunction {
        call_mode: scoop_identity::CAbiCallMode::NativeSafe,
        identity: scoop_lir::ExternFunctionIdentity {
            source_name: "consume".to_string(),
            native_symbol: "native_consume".to_string(),
            library: "fixture".to_string(),
            calling_convention: scoop_lir::CallingConvention::Cdecl,
        },
        bridge: outbound_bridge(4),
        signature: scoop_lir::CFunctionType {
            params: vec![parameter],
            return_type: scoop_lir::CReturnType::Void,
        },
    });
    let caller = &mut module.functions[0];
    let storage = caller.locals.alloc(test_local("c_argument", storage_type));
    let site = void_site(
        &mut caller.call_targets,
        TestCallProtocol::NativeSafe {
            safepoint: 99,
            destination: scoop_lir::CCallDestination::extern_function(function),
        },
        vec![RAW_PTR],
        vec![argument(storage)],
    );
    caller.blocks[caller.entry]
        .instructions
        .push(Instruction::Call { site });
    refresh_test_safepoints(caller);
}

#[test]
fn c_extern_call_requires_dedicated_argument_storage_addresses() {
    let mut module = values_module();
    append_c_void_call(
        &mut module,
        scoop_lir::CType::Integer(IntegerKind::SIGNED_8),
        LirType::I8,
        |_| Value::NullPointer(PointerKind::Raw),
    );

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("a raw pointer is not an exact C argument-storage witness");
    assert!(
        error
            .0
            .contains("argument 0 is not an exact C argument-storage address"),
        "unexpected error: {error}"
    );
}

#[test]
fn c_extern_call_binds_each_argument_to_its_exact_c_storage_type() {
    let mut module = values_module();
    append_c_void_call(
        &mut module,
        scoop_lir::CType::Integer(IntegerKind::SIGNED_8),
        LirType::I64,
        |storage| Value::CArgumentStorage(scoop_lir::CArgumentStorage::address_of(storage)),
    );

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("the C argument local has the wrong exact storage type");
    assert!(
        error
            .0
            .contains("argument 0 storage local3 has type i64, expected exact i8"),
        "unexpected error: {error}"
    );
}

#[test]
fn c_argument_storage_address_cannot_escape_to_another_call_protocol() {
    let mut module = values_module();
    let caller = &mut module.functions[0];
    let storage = caller
        .locals
        .alloc(test_local("escaped_c_argument", LirType::I8));
    let site = void_site(
        &mut caller.call_targets,
        TestCallProtocol::NoGc {
            destination: no_gc_runtime(scoop_lir::NoGcRuntimeFunction::Trap),
        },
        vec![RAW_PTR],
        vec![Value::CArgumentStorage(
            scoop_lir::CArgumentStorage::address_of(storage),
        )],
    );
    caller.blocks[caller.entry]
        .instructions
        .push(Instruction::Call { site });

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("C argument storage is not a general raw-pointer operand");
    assert!(
        error
            .0
            .contains("uses a C argument-storage address outside a C extern call"),
        "unexpected error: {error}"
    );
}

#[test]
fn exact_c_argument_storage_address_reaches_the_bridge_as_its_backing_alloca() {
    let mut module = values_module();
    append_c_void_call(
        &mut module,
        scoop_lir::CType::Integer(IntegerKind::SIGNED_8),
        LirType::I8,
        |storage| Value::CArgumentStorage(scoop_lir::CArgumentStorage::address_of(storage)),
    );

    let bridge_symbol = match &module
        .extern_functions
        .iter()
        .next()
        .expect("C extern")
        .1
        .kind
    {
        scoop_lir::ExternFunctionKind::C { bridge, .. } => bridge.symbol().to_string(),
        scoop_lir::ExternFunctionKind::Scoop { .. } => panic!("expected C extern"),
    };
    let ir = ir_of(&module);
    assert!(
        ir.lines()
            .any(|line| line.contains(&format!("call void @\"{bridge_symbol}\"(ptr %c_argument)"))),
        "C bridge did not receive the exact backing alloca:\n{ir}"
    );
}

#[test]
fn native_global_derives_physical_storage_from_exact_c_type() {
    let mut module = values_module();
    install_test_native_global_contract(&mut module, 1);
    let get = module
        .native_global_bridges
        .gets
        .alloc(scoop_lir::NativeGlobalGetBridge {
            identity: global_read_bridge(1),
        });
    let address =
        module
            .native_global_bridges
            .addresses
            .alloc(scoop_lir::NativeGlobalAddressBridge {
                identity: global_address_bridge(1),
            });
    module.native_globals.alloc(scoop_lir::NativeGlobal {
        source_name: "machineGlobal".to_string(),
        native_symbol: "machine_global".to_string(),
        library: "fixture".to_string(),
        c_type: scoop_lir::CType::Integer(IntegerKind::UNSIGNED_64),
        thread_local: false,
        access: scoop_lir::NativeGlobalAccess::ReadOnly { get, address },
    });

    let (_, global) = module
        .native_globals
        .iter()
        .next()
        .expect("one native global");
    assert_eq!(global.storage_type(), LirType::I64);
    let bridge = joined_c_bridge_sources(&module).expect("native global emits bridge sources");
    assert!(bridge.contains("extern uint64_t machine_global;"));
}

#[test]
fn equivalent_native_global_contracts_share_generated_bridge_definitions() {
    let mut module = values_module();
    install_test_native_global_contract(&mut module, 9);
    let read_identity = global_read_bridge(9);
    let read_symbol = read_identity.symbol().to_string();
    let address_identity = global_address_bridge(9);
    let address_symbol = address_identity.symbol().to_string();

    for source_name in ["firstGlobal", "secondGlobal"] {
        let get = module
            .native_global_bridges
            .gets
            .alloc(scoop_lir::NativeGlobalGetBridge {
                identity: read_identity.clone(),
            });
        let address =
            module
                .native_global_bridges
                .addresses
                .alloc(scoop_lir::NativeGlobalAddressBridge {
                    identity: address_identity.clone(),
                });
        module.native_globals.alloc(scoop_lir::NativeGlobal {
            source_name: source_name.to_string(),
            native_symbol: "shared_global".to_string(),
            library: "fixture".to_string(),
            c_type: scoop_lir::CType::Integer(IntegerKind::SIGNED_64),
            thread_local: false,
            access: scoop_lir::NativeGlobalAccess::ReadOnly { get, address },
        });
    }

    let bridge = joined_c_bridge_sources(&module).expect("native global needs bridge sources");
    assert_eq!(
        bridge
            .matches(&format!("void {read_symbol}(void *result)"))
            .count(),
        1,
        "one native contract must emit one read bridge:\n{bridge}"
    );
    assert_eq!(
        bridge
            .matches(&format!("void {address_symbol}(void *result)"))
            .count(),
        1,
        "one native contract must emit one address bridge:\n{bridge}"
    );
}

#[test]
fn refined_nullable_references_cannot_cross_raw_and_code_provenance() {
    let mut module = values_module();
    let raw = pointer_niche(
        &mut module,
        "Option<Ptr<Unit>>",
        scoop_lir::NichePointerKind::Raw,
    );
    let code = pointer_niche(
        &mut module,
        "Option<FunPtr<() -> Unit>>",
        scoop_lir::NichePointerKind::Code,
    );
    let pointee = scoop_lir::CDataPointee::OpaqueVoid;
    let signature = scoop_lir::CFunctionType {
        params: Vec::new(),
        return_type: scoop_lir::CReturnType::Void,
    };

    assert!(
        module
            .enums
            .nullable_data_pointer_ref(raw, pointee.clone())
            .is_some()
    );
    assert!(
        module
            .enums
            .nullable_code_pointer_ref(raw, signature.clone())
            .is_none()
    );
    assert!(
        module
            .enums
            .nullable_code_pointer_ref(code, signature)
            .is_some()
    );
    assert!(
        module
            .enums
            .nullable_data_pointer_ref(code, pointee)
            .is_none()
    );
}

#[test]
fn nullable_data_pointer_ref_rejects_a_mismatched_exact_pointee() {
    let mut module = values_module();
    let option = pointer_niche(
        &mut module,
        "Option<Ptr<Unit>>",
        scoop_lir::NichePointerKind::Raw,
    );
    let reference = module
        .enums
        .nullable_data_pointer_ref(option, scoop_lir::CDataPointee::OpaqueVoid)
        .expect("test raw-pointer niche");
    module.structs.alloc_c(
        crate::tests::test_physical_exact(
            "MalformedNullableData",
            scoop_identity::SourceNominalKind::Struct,
        ),
        "MalformedNullableData".to_string(),
        8,
        8,
        false,
        scoop_lir::LirCLayoutContract {
            aligned: scoop_lir::LirCLayoutValue::Natural,
            packed: scoop_lir::LirCLayoutValue::Natural,
        },
        vec![scoop_lir::CStructField {
            identity: test_field_identity("MalformedNullableData", "value"),
            ty: scoop_lir::CType::DataPointer {
                pointee: scoop_lir::CDataPointee::Object(Box::new(scoop_lir::CType::Integer(
                    IntegerKind::SIGNED_32,
                ))),
                storage: scoop_lir::CDataPointerStorage::Nullable(reference),
            },
            layout: scoop_lir::FieldLayout {
                offset: 0,
                access_align: 8,
            },
        }],
    );

    let error = c_layout_assertions(&module).expect_err("mismatched exact pointee must fail");
    assert!(
        error.0.contains("binds a different exact pointee"),
        "{}",
        error.0
    );
}

#[test]
fn nullable_code_pointer_ref_rejects_a_mismatched_exact_signature() {
    let mut module = values_module();
    let option = pointer_niche(
        &mut module,
        "Option<FunPtr<() -> Unit>>",
        scoop_lir::NichePointerKind::Code,
    );
    let bound_signature = scoop_lir::CFunctionType {
        params: Vec::new(),
        return_type: scoop_lir::CReturnType::Void,
    };
    let reference = module
        .enums
        .nullable_code_pointer_ref(option, bound_signature)
        .expect("test code-pointer niche");
    module.structs.alloc_c(
        crate::tests::test_physical_exact(
            "MalformedNullableCode",
            scoop_identity::SourceNominalKind::Struct,
        ),
        "MalformedNullableCode".to_string(),
        8,
        8,
        false,
        scoop_lir::LirCLayoutContract {
            aligned: scoop_lir::LirCLayoutValue::Natural,
            packed: scoop_lir::LirCLayoutValue::Natural,
        },
        vec![scoop_lir::CStructField {
            identity: test_field_identity("MalformedNullableCode", "value"),
            ty: scoop_lir::CType::CodePointer {
                signature: Box::new(scoop_lir::CFunctionType {
                    params: vec![scoop_lir::CType::Integer(IntegerKind::SIGNED_32)],
                    return_type: scoop_lir::CReturnType::Void,
                }),
                storage: scoop_lir::CCodePointerStorage::Nullable(reference),
            },
            layout: scoop_lir::FieldLayout {
                offset: 0,
                access_align: 8,
            },
        }],
    );

    let error = c_layout_assertions(&module).expect_err("mismatched exact signature must fail");
    assert!(
        error.0.contains("binds a different exact signature"),
        "{}",
        error.0
    );
}

#[test]
fn c_layout_pointer_spelling_follows_niche_provenance() {
    let mut module = values_module();
    let raw = pointer_niche(
        &mut module,
        "Option<Ptr<Unit>>",
        scoop_lir::NichePointerKind::Raw,
    );
    let code = pointer_niche(
        &mut module,
        "Option<FunPtr<() -> Unit>>",
        scoop_lir::NichePointerKind::Code,
    );
    let raw_type = c_nullable_opaque_pointer(&module.enums, raw);
    let code_type = c_nullable_function_pointer(&module.enums, code);
    module.structs.alloc_c(
        crate::tests::test_physical_exact(
            "PointerFields",
            scoop_identity::SourceNominalKind::Struct,
        ),
        "PointerFields".to_string(),
        16,
        8,
        false,
        scoop_lir::LirCLayoutContract {
            aligned: scoop_lir::LirCLayoutValue::A8,
            packed: scoop_lir::LirCLayoutValue::Natural,
        },
        vec![
            scoop_lir::CStructField {
                identity: test_field_identity("PointerFields", "raw"),
                ty: raw_type,
                layout: scoop_lir::FieldLayout {
                    offset: 0,
                    access_align: 8,
                },
            },
            scoop_lir::CStructField {
                identity: test_field_identity("PointerFields", "code"),
                ty: code_type,
                layout: scoop_lir::FieldLayout {
                    offset: 8,
                    access_align: 8,
                },
            },
        ],
    );

    let assertions = c_layout_assertions(&module).expect("raw/code niches are valid C fields");
    assert!(assertions.contains("void *_field_0;"), "{assertions}");
    assert!(
        assertions.contains("scoop_c_funptr_0 _field_1;"),
        "{assertions}"
    );
    let source = std::env::temp_dir().join(format!(
        "scoop_niche_pointer_layout_{}.c",
        std::process::id()
    ));
    std::fs::write(&source, &assertions).expect("write generated pointer-niche C");
    let status = std::process::Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-fsyntax-only"])
        .arg(&source)
        .status()
        .expect("compile generated pointer-niche C");
    std::fs::remove_file(&source).ok();
    assert!(status.success(), "generated pointer-niche C must compile");
}

#[test]
fn exact_c_pointer_tree_survives_fields_functions_and_globals() {
    let mut module = values_module();
    let raw = pointer_niche(
        &mut module,
        "Option<Ptr<Unit>>",
        scoop_lir::NichePointerKind::Raw,
    );
    let code = pointer_niche(
        &mut module,
        "Option<FunPtr<(Int) -> Unit>>",
        scoop_lir::NichePointerKind::Code,
    );
    let callback_signature = scoop_lir::CFunctionType {
        params: vec![scoop_lir::CType::Integer(IntegerKind::SIGNED_32)],
        return_type: scoop_lir::CReturnType::Void,
    };
    let direct_data = c_opaque_pointer();
    let nullable_data = c_nullable_opaque_pointer(&module.enums, raw);
    let direct_code = scoop_lir::CType::CodePointer {
        signature: Box::new(callback_signature.clone()),
        storage: scoop_lir::CCodePointerStorage::Direct,
    };
    let nullable_code = scoop_lir::CType::CodePointer {
        signature: Box::new(callback_signature.clone()),
        storage: scoop_lir::CCodePointerStorage::Nullable(
            module
                .enums
                .nullable_code_pointer_ref(code, callback_signature)
                .expect("test code-pointer niche"),
        ),
    };
    assert!(matches!(
        &direct_data,
        scoop_lir::CType::DataPointer {
            storage: scoop_lir::CDataPointerStorage::Direct,
            ..
        }
    ));
    assert!(matches!(
        &nullable_data,
        scoop_lir::CType::DataPointer {
            storage: scoop_lir::CDataPointerStorage::Nullable(reference),
            ..
        } if reference.definition() == raw
    ));
    assert!(matches!(
        &direct_code,
        scoop_lir::CType::CodePointer {
            storage: scoop_lir::CCodePointerStorage::Direct,
            ..
        }
    ));
    assert!(matches!(
        &nullable_code,
        scoop_lir::CType::CodePointer {
            storage: scoop_lir::CCodePointerStorage::Nullable(reference),
            ..
        } if reference.definition() == code
    ));

    let pointer_types = [
        ("direct_data", direct_data),
        ("nullable_data", nullable_data),
        ("direct_code", direct_code),
        ("nullable_code", nullable_code),
    ];
    module.structs.alloc_c(
        crate::tests::test_physical_exact("PointerTree", scoop_identity::SourceNominalKind::Struct),
        "PointerTree".to_string(),
        32,
        8,
        false,
        scoop_lir::LirCLayoutContract {
            aligned: scoop_lir::LirCLayoutValue::A8,
            packed: scoop_lir::LirCLayoutValue::Natural,
        },
        pointer_types
            .iter()
            .enumerate()
            .map(|(index, (_, ty))| scoop_lir::CStructField {
                identity: test_field_identity("PointerTree", &format!("field{index}")),
                ty: ty.clone(),
                layout: scoop_lir::FieldLayout {
                    offset: u64::try_from(index).expect("four test fields") * 8,
                    access_align: 8,
                },
            })
            .collect(),
    );
    for (name, ty) in &pointer_types {
        let function_seed = u8::try_from(module.extern_functions.iter().count() + 10).unwrap();
        install_test_native_function_contract(&mut module, function_seed);
        module.extern_functions.alloc_c(scoop_lir::CExternFunction {
            call_mode: scoop_identity::CAbiCallMode::NativeSafe,
            identity: scoop_lir::ExternFunctionIdentity {
                source_name: format!("roundtrip{name}"),
                native_symbol: format!("roundtrip_{name}"),
                library: "fixture".to_string(),
                calling_convention: scoop_lir::CallingConvention::Cdecl,
            },
            bridge: outbound_bridge(function_seed),
            signature: scoop_lir::CFunctionType {
                params: vec![ty.clone()],
                return_type: c_value(ty.clone()),
            },
        });
        let global_seed = u8::try_from(module.native_globals.len() + 40).unwrap();
        install_test_native_global_contract(&mut module, global_seed);
        let get = module
            .native_global_bridges
            .gets
            .alloc(scoop_lir::NativeGlobalGetBridge {
                identity: global_read_bridge(global_seed),
            });
        let address =
            module
                .native_global_bridges
                .addresses
                .alloc(scoop_lir::NativeGlobalAddressBridge {
                    identity: global_address_bridge(global_seed),
                });
        module.native_globals.alloc(scoop_lir::NativeGlobal {
            source_name: format!("global{name}"),
            native_symbol: format!("global_{name}"),
            library: "fixture".to_string(),
            c_type: ty.clone(),
            thread_local: false,
            access: scoop_lir::NativeGlobalAccess::ReadOnly { get, address },
        });
    }

    let assertions = c_layout_assertions(&module).expect("pointer C layout assertions");
    assert!(assertions.contains("void *_field_0;"), "{assertions}");
    assert!(assertions.contains("void *_field_1;"), "{assertions}");
    assert!(
        assertions.contains("scoop_c_funptr_0 _field_2;"),
        "{assertions}"
    );
    assert!(
        assertions.contains("scoop_c_funptr_0 _field_3;"),
        "{assertions}"
    );
    let bridge_sources = crate::c_bridge::render_c_bridge_source_set_for_module(&module)
        .expect("pointer externs and globals emit bridge sources");
    let source = bridge_sources
        .units()
        .iter()
        .map(|unit| unit.source())
        .collect::<Vec<_>>()
        .join("\n");
    for name in ["direct_data", "nullable_data"] {
        assert!(
            source.contains(&format!("extern void *roundtrip_{name}(void *);")),
            "{source}"
        );
        assert!(
            source.contains(&format!("extern void *global_{name};")),
            "{source}"
        );
    }
    for name in ["direct_code", "nullable_code"] {
        assert!(
            source.contains(&format!(
                "extern scoop_c_funptr_0 roundtrip_{name}(scoop_c_funptr_0);"
            )),
            "{source}"
        );
        assert!(
            source.contains(&format!("extern scoop_c_funptr_0 global_{name};")),
            "{source}"
        );
    }

    for unit in bridge_sources.units() {
        let bridge_source = std::env::temp_dir().join(format!(
            "scoop_exact_c_pointer_tree_{}_{}.c",
            std::process::id(),
            unit.unit()
        ));
        std::fs::write(&bridge_source, unit.source()).expect("write exact C pointer bridge");
        let status = std::process::Command::new("cc")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-fsyntax-only"])
            .arg(&bridge_source)
            .status()
            .expect("compile exact C pointer bridge");
        std::fs::remove_file(&bridge_source).ok();
        assert!(status.success(), "exact C pointer bridge must compile");
    }
}

#[test]
fn c_layout_exact_declarators_support_recursive_struct_and_function_pointers() {
    let mut module = values_module();
    let node = module.structs.alloc_c(
        crate::tests::test_physical_exact("Node", scoop_identity::SourceNominalKind::Struct),
        "Node".to_string(),
        16,
        8,
        false,
        scoop_lir::LirCLayoutContract {
            aligned: scoop_lir::LirCLayoutValue::Natural,
            packed: scoop_lir::LirCLayoutValue::Natural,
        },
        Vec::new(),
    );
    let node_id = node.definition();
    let node_pointer = scoop_lir::CType::DataPointer {
        pointee: scoop_lir::CDataPointee::Object(Box::new(c_struct(&module.structs, node_id))),
        storage: scoop_lir::CDataPointerStorage::Direct,
    };
    let visitor = scoop_lir::CType::CodePointer {
        signature: Box::new(scoop_lir::CFunctionType {
            params: vec![c_struct(&module.structs, node_id)],
            return_type: scoop_lir::CReturnType::Void,
        }),
        storage: scoop_lir::CCodePointerStorage::Direct,
    };
    module.structs.set_c_fields(
        node,
        vec![
            scoop_lir::CStructField {
                identity: test_field_identity("Node", "next"),
                ty: node_pointer,
                layout: scoop_lir::FieldLayout {
                    offset: 0,
                    access_align: 8,
                },
            },
            scoop_lir::CStructField {
                identity: test_field_identity("Node", "visitor"),
                ty: visitor,
                layout: scoop_lir::FieldLayout {
                    offset: 8,
                    access_align: 8,
                },
            },
        ],
    );

    let assertions = c_layout_assertions(&module).expect("recursive pointer layout is valid");
    let forward = assertions
        .find("typedef struct scoop_c_layout_0 scoop_c_layout_0;")
        .expect("C struct forward declaration");
    let function_pointer = assertions
        .find("typedef void (*scoop_c_funptr_0)(scoop_c_layout_0 arg0);")
        .expect("exact recursive by-value function-pointer typedef");
    let definition = assertions
        .find("struct __attribute__((packed, aligned(8))) scoop_c_layout_0 {")
        .expect("C struct definition");
    assert!(forward < function_pointer && function_pointer < definition);
    assert!(
        assertions.contains("scoop_c_layout_0 *_field_0;"),
        "{assertions}"
    );
    assert!(
        assertions.contains("scoop_c_funptr_0 _field_1;"),
        "{assertions}"
    );

    let source =
        std::env::temp_dir().join(format!("scoop_recursive_c_layout_{}.c", std::process::id()));
    std::fs::write(&source, &assertions).expect("write recursive C layout");
    let status = std::process::Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-fsyntax-only"])
        .arg(&source)
        .status()
        .expect("compile recursive C layout");
    std::fs::remove_file(&source).ok();
    assert!(
        status.success(),
        "generated recursive C layout must compile"
    );
}

#[test]
fn c_layout_rejects_a_mutual_by_value_struct_cycle() {
    let mut module = values_module();
    let a = module.structs.alloc_c(
        crate::tests::test_physical_exact("A", scoop_identity::SourceNominalKind::Struct),
        "A".to_string(),
        1,
        1,
        false,
        scoop_lir::LirCLayoutContract {
            aligned: scoop_lir::LirCLayoutValue::Natural,
            packed: scoop_lir::LirCLayoutValue::Natural,
        },
        Vec::new(),
    );
    let b = module.structs.alloc_c(
        crate::tests::test_physical_exact("B", scoop_identity::SourceNominalKind::Struct),
        "B".to_string(),
        1,
        1,
        false,
        scoop_lir::LirCLayoutContract {
            aligned: scoop_lir::LirCLayoutValue::Natural,
            packed: scoop_lir::LirCLayoutValue::Natural,
        },
        Vec::new(),
    );
    module.structs.set_c_fields(
        a,
        vec![scoop_lir::CStructField {
            identity: test_field_identity("A", "b"),
            ty: scoop_lir::CType::Struct(b),
            layout: scoop_lir::FieldLayout {
                offset: 0,
                access_align: 1,
            },
        }],
    );
    module.structs.set_c_fields(
        b,
        vec![scoop_lir::CStructField {
            identity: test_field_identity("B", "a"),
            ty: scoop_lir::CType::Struct(a),
            layout: scoop_lir::FieldLayout {
                offset: 0,
                access_align: 1,
            },
        }],
    );

    let error = c_layout_assertions(&module).expect_err("by-value C cycles are invalid");
    assert!(
        error.0.contains("recursively embedded by value")
            && error.0.contains("struct `A`")
            && error.0.contains("struct `B`"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn c_callback_declarations_apply_exact_narrow_integer_abi_extensions() {
    let mut module = values_module();
    let signed_bridge = static_callback_bridge(&mut module, "signed_narrow_bridge");
    install_test_c_signature(&mut module, 10);
    let signed_trampoline = static_callback_trampoline(10);
    module.callback_bridges.alloc(scoop_lir::CallbackBridge {
        source_name: "signedNarrow".to_string(),
        bridge: scoop_lir::StaticCallbackTarget::Local(signed_bridge),
        trampoline: signed_trampoline,
        params: vec![
            scoop_lir::CType::Integer(IntegerKind::SIGNED_8),
            scoop_lir::CType::Integer(IntegerKind::UNSIGNED_8),
            scoop_lir::CType::Integer(IntegerKind::SIGNED_16),
            scoop_lir::CType::Integer(IntegerKind::UNSIGNED_16),
            scoop_lir::CType::Boolean,
            scoop_lir::CType::Integer(IntegerKind::SIGNED_32),
            scoop_lir::CType::Integer(IntegerKind::UNSIGNED_32),
            scoop_lir::CType::Integer(IntegerKind::SIGNED_64),
            scoop_lir::CType::Integer(IntegerKind::UNSIGNED_64),
        ],
        return_type: c_value(scoop_lir::CType::Integer(IntegerKind::SIGNED_8)),
    });
    let unsigned_bridge = static_callback_bridge(&mut module, "unsigned_narrow_bridge");
    install_test_c_signature(&mut module, 11);
    let unsigned_trampoline = static_callback_trampoline(11);
    module.callback_bridges.alloc(scoop_lir::CallbackBridge {
        source_name: "unsignedNarrow".to_string(),
        bridge: scoop_lir::StaticCallbackTarget::Local(unsigned_bridge),
        trampoline: unsigned_trampoline,
        params: Vec::new(),
        return_type: c_value(scoop_lir::CType::Integer(IntegerKind::UNSIGNED_16)),
    });
    let bool_bridge = static_callback_bridge(&mut module, "bool_narrow_bridge");
    install_test_c_signature(&mut module, 12);
    let bool_trampoline = static_callback_trampoline(12);
    module.callback_bridges.alloc(scoop_lir::CallbackBridge {
        source_name: "boolNarrow".to_string(),
        bridge: scoop_lir::StaticCallbackTarget::Local(bool_bridge),
        trampoline: bool_trampoline,
        params: Vec::new(),
        return_type: c_value(scoop_lir::CType::Boolean),
    });

    let family = foreign_callback_family(&mut module);
    add_foreign_callback_bridge(
        &mut module,
        family,
        ForeignCallbackBridgeFixture {
            adapter: "foreign_narrow_adapter",
            identity_seed: 1,
            params: vec![
                c_opaque_pointer(),
                scoop_lir::CType::Integer(IntegerKind::SIGNED_16),
                scoop_lir::CType::Boolean,
            ],
            return_type: c_value(scoop_lir::CType::Integer(IntegerKind::UNSIGNED_8)),
            context_index: 0,
        },
    );
    let foreign_bridge_symbol = module
        .foreign_callback_bridges
        .iter()
        .next()
        .expect("foreign callback bridge")
        .1
        .trampoline
        .entry()
        .symbol()
        .to_string();
    let mut static_symbols = module
        .callback_bridges
        .iter()
        .map(|(_, bridge)| bridge.trampoline.entry().symbol());
    let signed_symbol = static_symbols.next().expect("signed callback");
    let unsigned_symbol = static_symbols.next().expect("unsigned callback");
    let bool_symbol = static_symbols.next().expect("boolean callback");
    assert!(static_symbols.next().is_none());

    let ir = ir_of(&module);
    assert!(
        ir.contains(&format!(
            "declare signext i8 @\"{signed_symbol}\"(i8 signext, i8 zeroext, i16 signext, i16 zeroext, i1 zeroext, i32, i32, i64, i64)"
        )),
        "static callback parameters lost signedness/width ABI attributes:\n{ir}"
    );
    assert!(
        ir.contains(&format!("declare zeroext i16 @\"{unsigned_symbol}\"()")),
        "unsigned narrow callback result lost zeroext:\n{ir}"
    );
    assert!(
        ir.contains(&format!("declare zeroext i1 @\"{bool_symbol}\"()")),
        "C _Bool callback result lost zeroext:\n{ir}"
    );
    assert!(
        ir.contains(&format!(
            "declare zeroext i8 @\"{foreign_bridge_symbol}\"(ptr, i16 signext, i1 zeroext)"
        )),
        "foreign callback boundary lost narrow integer ABI attributes:\n{ir}"
    );
}

#[test]
fn refined_c_pointer_references_reject_managed_niches() {
    let mut module = values_module();
    let managed = pointer_niche(
        &mut module,
        "Option<String>",
        scoop_lir::NichePointerKind::Managed,
    );
    assert!(
        module
            .enums
            .nullable_data_pointer_ref(managed, scoop_lir::CDataPointee::OpaqueVoid)
            .is_none()
    );
    assert!(
        module
            .enums
            .nullable_code_pointer_ref(
                managed,
                scoop_lir::CFunctionType {
                    params: Vec::new(),
                    return_type: scoop_lir::CReturnType::Void,
                },
            )
            .is_none()
    );
}

#[test]
fn foreign_callback_bridge_rejects_wrong_adapter_signature() {
    let mut module = values_module();
    let family = foreign_callback_family(&mut module);
    module
        .foreign_callback_bridges
        .alloc(scoop_lir::ForeignCallbackBridge {
            application: callback_application(0),
            family,
            adapter: managed_local_function_ref(0),
            trampoline: callback_trampoline(5, 0),
            params: vec![c_opaque_pointer()],
            return_type: scoop_lir::CReturnType::Void,
            context_index: 0,
            mode: module.foreign_callback_families[family].modes.reusable(),
        });

    let error = joined_c_bridge_sources(&module)
        .expect_err("foreign callback bridge must point at an exact managed adapter");
    assert!(
        error.0.contains(&format!(
            "foreign callback adapter @{}",
            module.functions[0].symbol()
        )) && error.0.contains("machine<foreign-callback-status>"),
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

    let error = crate::validation::validate_module(&module)
        .expect_err("callback operations must preserve nominal callback identity");
    assert!(
        error.0.contains("requires exact callback struct")
            && error.0.contains(&format!("struct{}", lookalike.into_raw())),
        "unexpected error: {error}"
    );
}

#[test]
fn foreign_callback_family_revalidates_mode_state_and_failure_metadata() {
    type CorruptCallbackFamily = fn(&mut Module, scoop_lir::ForeignCallbackFamilyId);
    let cases: [(&str, CorruptCallbackFamily); 3] = [
        (
            "mode",
            |module: &mut Module, family: scoop_lir::ForeignCallbackFamilyId| {
                let mut foreign = values_module();
                callback_state(&mut foreign, "Padding");
                let definition = callback_mode(&mut foreign);
                let modes = scoop_lir::ForeignCallbackModes::checked(
                    &foreign.enums,
                    foreign.enums.variant_ref(definition, 0).unwrap(),
                    foreign.enums.variant_ref(definition, 1).unwrap(),
                )
                .unwrap();
                module.foreign_callback_families[family].modes = modes;
            },
        ),
        (
            "state",
            |module: &mut Module, family: scoop_lir::ForeignCallbackFamilyId| {
                let mut foreign = values_module();
                let definition = callback_state(&mut foreign, "ForeignCallbackState");
                let states = scoop_lir::ForeignCallbackStates::checked(
                    &foreign.enums,
                    foreign.enums.variant_ref(definition, 0).unwrap(),
                    foreign.enums.variant_ref(definition, 1).unwrap(),
                    foreign.enums.variant_ref(definition, 2).unwrap(),
                    foreign.enums.variant_ref(definition, 3).unwrap(),
                )
                .unwrap();
                module.foreign_callback_families[family].states = states;
            },
        ),
        (
            "managed-reference callback failure",
            |module: &mut Module, family: scoop_lir::ForeignCallbackFamilyId| {
                let mut foreign = values_module();
                let definition = callback_failure(&mut foreign, "Option<Throwable>");
                let some = foreign.enums.variant_ref(definition, 0).unwrap();
                let failure = scoop_lir::ForeignCallbackFailureResult::checked(
                    &foreign.enums,
                    foreign.enums.variant_field_ref(some, 0).unwrap(),
                    foreign.enums.variant_ref(definition, 1).unwrap(),
                )
                .unwrap();
                module.foreign_callback_families[family].failure_result = failure;
            },
        ),
    ];
    for (kind, corrupt) in cases {
        let mut module = values_module();
        let family = foreign_callback_family(&mut module);
        corrupt(&mut module, family);

        let error = joined_c_bridge_sources(&module)
            .expect_err("callback family metadata must be revalidated at codegen entry");
        assert!(error.0.contains(kind), "unexpected error: {error}");
    }
}

#[test]
fn foreign_callback_bridge_mode_must_belong_to_its_family() {
    let mut module = values_module();
    let family = foreign_callback_family(&mut module);
    let foreign_mode = module.foreign_callback_families[family].states.registered();
    add_foreign_callback_bridge(
        &mut module,
        family,
        ForeignCallbackBridgeFixture {
            adapter: "foreign_callback_adapter",
            identity_seed: 1,
            params: vec![c_opaque_pointer()],
            return_type: scoop_lir::CReturnType::Void,
            context_index: 0,
        },
    );
    let bridge = module
        .foreign_callback_bridges
        .iter()
        .next()
        .expect("test bridge")
        .0;
    module.foreign_callback_bridges[bridge].mode = foreign_mode;

    let error = joined_c_bridge_sources(&module)
        .expect_err("callback bridge mode must belong to its typed family");
    assert!(
        error.0.contains("mode outside family"),
        "unexpected error: {error}"
    );
}

#[test]
fn foreign_callback_state_decodes_only_the_closed_wire_codes() {
    let mut module = values_module();
    let family = foreign_callback_family(&mut module);
    let callback = module.foreign_callback_families[family].callback;
    let states = module.foreign_callback_families[family].states;
    let signature = scoop_signature(
        &module.structs,
        &module.enums,
        vec![LirType::Struct(callback)],
        LirType::Void,
    );
    let function = &mut module.functions[0];
    function.signature = signature;
    let out = function.temps.alloc(Temp {
        ty: LirType::Enum(states.definition()),
    });
    function.blocks[function.entry]
        .instructions
        .push(Instruction::ForeignCallbackOperation(
            scoop_lir::ForeignCallbackOperation::State {
                family,
                out,
                callback: Value::Param(0),
            },
        ));

    let ir = ir_of(&module);
    for expected in [
        "icmp ule i32 %callback_state, 3",
        "label %callback_state_invalid",
        "call void @llvm.trap()",
        "unreachable",
        "select i1 %callback_state_case, i64 2, i64 3",
        "i64 1, i64 %callback_state_tag",
        "i64 0, i64 %callback_state_tag",
    ] {
        assert!(ir.contains(expected), "missing `{expected}` in:\n{ir}");
    }
    assert_eq!(
        ir.matches("select i1 %callback_state_case").count(),
        3,
        "{ir}"
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

    let error = crate::validation::validate_module(&module)
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

    let error = crate::validation::validate_module(&module)
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
            identity_seed: 1,
            params: vec![c_opaque_pointer()],
            return_type: scoop_lir::CReturnType::Void,
            context_index: 1,
        },
    );

    let error =
        joined_c_bridge_sources(&module).expect_err("context index must name a C parameter");
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
            identity_seed: 1,
            params: vec![scoop_lir::CType::Integer(IntegerKind::SIGNED_64)],
            return_type: scoop_lir::CReturnType::Void,
            context_index: 0,
        },
    );

    let error = joined_c_bridge_sources(&module).expect_err("callback context must be a C pointer");
    assert!(
        error
            .0
            .contains("context parameter 0 must be a direct opaque C data pointer"),
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
            identity_seed: 1,
            params: vec![c_opaque_pointer()],
            return_type: scoop_lir::CReturnType::Void,
            context_index: 0,
        },
    );
    add_foreign_callback_bridge(
        &mut module,
        family,
        ForeignCallbackBridgeFixture {
            adapter: "foreign_callback_adapter_1",
            identity_seed: 1,
            params: vec![
                c_opaque_pointer(),
                scoop_lir::CType::Integer(IntegerKind::SIGNED_64),
            ],
            return_type: c_value(scoop_lir::CType::Integer(IntegerKind::SIGNED_64)),
            context_index: 0,
        },
    );

    let symbol = module
        .foreign_callback_bridges
        .iter()
        .next()
        .unwrap()
        .1
        .trampoline
        .entry()
        .symbol()
        .to_string();
    let error = joined_c_bridge_sources(&module)
        .expect_err("a shared trampoline symbol must have one ABI description");
    assert!(
        error.0.contains(&format!(
            "trampoline symbol @{symbol} has conflicting ABI metadata"
        )),
        "unexpected error: {error}"
    );
}
