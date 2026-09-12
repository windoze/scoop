use super::*;

#[test]
fn storage_global_linkage_follows_its_materialization_root() {
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
                ty: LirType::I64,
                initial_state: LirStaticInitialState::EncodedStaticValue {
                    payload: LirConstantImage::Integer(scoop_lir::LirIntegerConstant::Signed64(0)),
                },
                thread_local: false,
            },
        });
    }

    let ir = ir_of(&module);
    assert!(
        ir.contains(&format!("@\"{cone_symbol}\" = global i64 0")),
        "Cone-owned storage must be a strong definition:\n{ir}"
    );
    assert!(
        ir.contains(&format!("@\"{odr_symbol}\" = weak_odr global i64 0")),
        "ODR-owned storage must be a coalescible definition:\n{ir}"
    );
}

#[test]
fn emits_typed_initialization_descriptors_in_persistent_identity_order() {
    let mut module = values_module();
    let storage = module.globals.alloc(Global {
        address_kind: PointerKind::Raw,
        scan: RefScan::None,
        init: GlobalInit::Storage {
            identity: static_storage_identity("initStorage"),
            ty: LirType::I64,
            initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
            thread_local: false,
        },
    });
    let failure = module.globals.alloc(Global {
        address_kind: PointerKind::Raw,
        scan: RefScan::References(vec![0]),
        init: GlobalInit::Storage {
            identity: static_storage_identity("initFailure"),
            ty: MANAGED_PTR,
            initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
            thread_local: false,
        },
    });
    let second_storage = module.globals.alloc(Global {
        address_kind: PointerKind::Raw,
        scan: RefScan::None,
        init: GlobalInit::Storage {
            identity: static_storage_identity("secondInitStorage"),
            ty: LirType::I64,
            initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
            thread_local: false,
        },
    });
    let second_failure = module.globals.alloc(Global {
        address_kind: PointerKind::Raw,
        scan: RefScan::References(vec![0]),
        init: GlobalInit::Storage {
            identity: static_storage_identity("secondInitFailure"),
            ty: MANAGED_PTR,
            initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
            thread_local: false,
        },
    });
    let mut functions = scoop_lir::LocalFunctionIdentities::default();
    let entry = functions.alloc_managed();
    let alpha = initialization_unit_identity(ConeIdentity::CORE, "alpha");
    let zed = initialization_unit_identity(ConeIdentity::SINGLE_FILE, "zed");
    let (higher_identity, higher_name, lower_identity, lower_name) = if alpha.id() > zed.id() {
        (alpha, "alpha", zed, "zed")
    } else {
        (zed, "zed", alpha, "alpha")
    };
    module
        .initialization_units
        .alloc(scoop_lir::InitializationUnit {
            identity: higher_identity,
            display_name: format!("top-level:{higher_name}"),
            schedule: scoop_lir::InitializationSchedule::EagerStartup,
            kind: scoop_lir::InitializationUnitKind::EagerTopLevel { storage },
            failure_root: failure,
            initializer: entry,
            ensure: entry,
            dependencies: Vec::new(),
        });
    module
        .initialization_units
        .alloc(scoop_lir::InitializationUnit {
            identity: lower_identity,
            display_name: format!("top-level:{lower_name}"),
            schedule: scoop_lir::InitializationSchedule::LazyAccess,
            kind: scoop_lir::InitializationUnitKind::EagerTopLevel {
                storage: second_storage,
            },
            failure_root: second_failure,
            initializer: entry,
            ensure: entry,
            dependencies: Vec::new(),
        });

    let ir = ir_of(&module);
    assert!(ir.contains("@scoop.init.cell.0 = private global { i64, ptr } zeroinitializer"));
    assert!(ir.contains("@scoop.init.descriptor.1 = private constant"));
    assert!(ir.contains("@scoop.init.display.0 = private constant"));
    assert!(ir.contains("@scoop.init.display.1 = private constant"));
    assert!(ir.contains(&format!("c\"top-level:{higher_name}\\00\"")));
    assert!(ir.contains(&format!("c\"top-level:{lower_name}\\00\"")));
    assert!(ir.contains("{ i64 0, [32 x i8] c\""));
    assert!(ir.contains("{ i64 1, [32 x i8] c\""));
    assert!(!ir.contains("scoop.init.key"));
    let table = ir
        .lines()
        .find(|line| line.starts_with("@scoop_image_initialization_units ="))
        .expect("initialization descriptor table");
    let lower = table
        .find("@scoop.init.display.1")
        .expect("lower persistent identity in table");
    let higher = table
        .find("@scoop.init.display.0")
        .expect("higher persistent identity in table");
    assert!(
        lower < higher,
        "descriptor table must be sorted by persistent identity, not arena order"
    );
    assert!(ir.contains("@scoop_image_initialization_unit_count = constant i64 2"));
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
            ty: LirType::MachineScalar(MachineScalarKind::InitializationOutcome),
            initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
            thread_local: false,
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
