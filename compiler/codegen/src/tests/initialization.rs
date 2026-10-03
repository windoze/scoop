use super::*;
use crate::emission::initialization_unit_globals_from_pairs;

#[test]
fn storage_codegen_uses_each_definition_owners_linkage() {
    let mut module = values_module();
    let cone_identity = static_storage_identity("coneStorage");
    let cone_symbol = cone_identity.symbol().to_string();
    let odr_identity = odr_static_storage_identity("odrStorage");
    let odr_symbol = odr_identity.symbol().to_string();

    for identity in [cone_identity, odr_identity] {
        module.globals.alloc(Global {
            address_kind: PointerKind::Raw,
            scan: RefScan::None,
            init: GlobalInit::Storage {
                identity,
                layout: layout_identity(
                    "storageGlobal",
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

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let llvm = emit_llvm_module(&context, &module, &machine, host_profile()).unwrap();
    for (symbol, linkage) in [
        (cone_symbol, inkwell::module::Linkage::External),
        (odr_symbol, inkwell::module::Linkage::WeakODR),
    ] {
        let storage = llvm.get_global(&symbol).unwrap();
        assert_eq!(storage.get_linkage(), linkage);
        assert!(storage.get_initializer().is_some());
    }
    llvm.verify().unwrap();
}

#[test]
fn zero_sized_storage_uses_one_addressable_byte() {
    let mut module = values_module();
    let identity = static_storage_identity("zeroSizedStorage");
    let symbol = identity.symbol().to_string();
    module.globals.alloc(Global {
        address_kind: PointerKind::Raw,
        scan: RefScan::None,
        init: GlobalInit::Storage {
            identity,
            layout: layout_identity(
                "zeroSizedStorage",
                scoop_identity::RepresentationRole::ManagedValue,
            )
            .into(),
            ty: LirType::Aggregate(Vec::new()),
            initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
        },
    });

    let ir = ir_of(&module);

    assert!(
        ir.contains(&format!(
            "@\"{symbol}\" = global i8 0, section \"__DATA,__bss\", align 1"
        )),
        "zero-sized storage must retain a unique address token:\n{ir}"
    );
}

#[test]
fn encoded_storage_is_forced_out_of_the_common_section() {
    let mut module = values_module();
    let identity = static_storage_identity("encodedZeroStorage");
    let symbol = identity.symbol().to_string();
    module.globals.alloc(Global {
        address_kind: PointerKind::Raw,
        scan: RefScan::None,
        init: GlobalInit::Storage {
            identity,
            layout: layout_identity(
                "encodedZeroStorage",
                scoop_identity::RepresentationRole::ManagedValue,
            )
            .into(),
            ty: LirType::I64,
            initial_state: LirStaticInitialState::EncodedStaticValue {
                payload: LirConstantImage::Integer(scoop_lir::LirIntegerConstant::Signed64(0)),
            },
        },
    });

    let ir = ir_of(&module);

    assert!(
        ir.contains(&format!(
            "@\"{symbol}\" = global i64 0, section \"__DATA,__data\", align 8"
        )),
        "encoded storage must be an explicit writable definition:\n{ir}"
    );
}

#[test]
fn unsealed_initialization_units_have_no_codegen_path() {
    let mut module = values_module();
    let storage = module.globals.alloc(Global {
        address_kind: PointerKind::Raw,
        scan: RefScan::None,
        init: GlobalInit::Storage {
            identity: static_storage_identity("initStorage"),
            layout: layout_identity(
                "initStorage",
                scoop_identity::RepresentationRole::ManagedValue,
            )
            .into(),
            ty: LirType::I64,
            initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
        },
    });
    let failure = module.globals.alloc(Global {
        address_kind: PointerKind::Raw,
        scan: RefScan::References(vec![0]),
        init: GlobalInit::Storage {
            identity: static_storage_identity("initFailure"),
            layout: layout_identity(
                "initFailure",
                scoop_identity::RepresentationRole::ManagedValue,
            )
            .into(),
            ty: MANAGED_PTR,
            initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
        },
    });
    let mut functions = scoop_lir::LocalFunctionIdentities::default();
    let entry = functions.alloc_managed();
    module
        .initialization_units
        .alloc(scoop_lir::InitializationUnit {
            identity: initialization_unit_identity(ConeIdentity::SINGLE_FILE, "value"),
            display_name: "top-level:value".to_string(),
            schedule: scoop_lir::InitializationSchedule::EagerStartup,
            kind: scoop_lir::InitializationUnitKind::EagerTopLevel { storage },
            failure_root: failure,
            initializer: entry,
            ensure: entry,
            dependencies: Vec::new(),
        });
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("initialization values require the closed production section");
    assert!(
        error
            .0
            .contains("initialization units require a sealed strong production section"),
        "{error}"
    );
}

#[test]
fn initialization_values_follow_typed_unit_ids_in_lir_arena_order() {
    let mut module = values_module();
    let first_identity = initialization_unit_identity(ConeIdentity::CORE, "first");
    let second_identity = initialization_unit_identity(ConeIdentity::SINGLE_FILE, "second");
    let first_id = first_identity.id();
    let second_id = second_identity.id();
    let storage = module.globals.alloc(Global {
        address_kind: PointerKind::Raw,
        scan: RefScan::None,
        init: GlobalInit::Storage {
            identity: static_storage_identity("mappingStorage"),
            layout: layout_identity(
                "mappingStorage",
                scoop_identity::RepresentationRole::ManagedValue,
            )
            .into(),
            ty: LirType::I64,
            initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
        },
    });
    let failure_root = module.globals.alloc(Global {
        address_kind: PointerKind::Raw,
        scan: RefScan::References(vec![0]),
        init: GlobalInit::Storage {
            identity: static_storage_identity("mappingFailure"),
            layout: layout_identity(
                "mappingFailure",
                scoop_identity::RepresentationRole::ManagedValue,
            )
            .into(),
            ty: MANAGED_PTR,
            initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
        },
    });
    for (identity, display_name) in [
        (first_identity, "top-level:first"),
        (second_identity, "top-level:second"),
    ] {
        module
            .initialization_units
            .alloc(scoop_lir::InitializationUnit {
                identity,
                display_name: display_name.to_string(),
                schedule: scoop_lir::InitializationSchedule::LazyAccess,
                kind: scoop_lir::InitializationUnitKind::EagerTopLevel { storage },
                failure_root,
                initializer: managed_local_function_ref(0),
                ensure: managed_local_function_ref(0),
                dependencies: Vec::new(),
            });
    }

    let context = Context::create();
    let llvm = context.create_module("initialization-id-mapping");
    let first = llvm.add_global(context.i8_type(), None, "first.coordinator");
    let second = llvm.add_global(context.i8_type(), None, "second.coordinator");
    let mapped =
        initialization_unit_globals_from_pairs(&module, [(second_id, second), (first_id, first)])
            .expect("typed coordinator ids cover the LIR arena");

    assert_eq!(mapped, [first, second]);
}

#[test]
fn storage_global_rejects_machine_scalar_type() {
    let mut module = values_module();
    let identity = static_storage_identity("machineGlobal");
    let symbol = identity.symbol().to_string();
    module.globals.alloc(Global {
        address_kind: PointerKind::Raw,
        scan: RefScan::None,
        init: GlobalInit::Storage {
            identity,
            layout: layout_identity(
                "machineGlobal",
                scoop_identity::RepresentationRole::ManagedValue,
            )
            .into(),
            ty: LirType::MachineScalar(MachineScalarKind::InitializationOutcome),
            initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
        },
    });

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("compiler-only scalar domains must not acquire global storage");
    assert!(
        error.0.contains(&format!("storage global `{symbol}`"))
            && error.0.contains("machine<initialization-outcome>"),
        "unexpected error: {error}"
    );
}
