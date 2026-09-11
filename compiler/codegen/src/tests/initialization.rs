use super::*;

#[test]
fn emits_typed_initialization_descriptors_in_stable_key_order() {
    let mut module = values_module();
    let storage = module.globals.alloc(Global {
        symbol: "scoop.init.storage".to_string(),
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
        symbol: "scoop.init.failure".to_string(),
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
        symbol: "scoop.init.storage.2".to_string(),
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
        symbol: "scoop.init.failure.2".to_string(),
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
    module
        .initialization_units
        .alloc(scoop_lir::InitializationUnit {
            identity: initialization_unit_identity(ConeIdentity::CORE, "alpha"),
            stable_key: "$local$opaque-z".to_string(),
            display_name: "top-level:alpha".to_string(),
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
            identity: initialization_unit_identity(ConeIdentity::SINGLE_FILE, "zed"),
            stable_key: "$local$opaque-a".to_string(),
            display_name: "top-level:zed".to_string(),
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
    assert!(ir.contains("c\"top-level:alpha\\00\""));
    assert!(ir.contains("c\"top-level:zed\\00\""));
    assert!(ir.contains("{ i64 0, ptr @scoop.init.key.0, ptr @scoop.init.display.0"));
    assert!(ir.contains("{ i64 1, ptr @scoop.init.key.1, ptr @scoop.init.display.1"));
    let table = ir
        .lines()
        .find(|line| line.starts_with("@scoop_image_initialization_units ="))
        .expect("initialization descriptor table");
    let opaque_a = table
        .find("@scoop.init.key.1")
        .expect("opaque a key in table");
    let opaque_z = table
        .find("@scoop.init.key.0")
        .expect("opaque z key in table");
    assert!(
        opaque_a < opaque_z,
        "descriptor table must be sorted by stable key, not display name"
    );
    assert!(ir.contains("@scoop_image_initialization_unit_count = constant i64 2"));
}

#[test]
fn storage_global_rejects_machine_scalar_type() {
    let mut module = values_module();
    module.globals.alloc(Global {
        symbol: "scoop.machine.global".to_string(),
        address_kind: PointerKind::Raw,
        scan: RefScan::None,
        init: GlobalInit::Storage {
            identity: static_storage_identity("machineGlobal"),
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
        error.0.contains("storage global `scoop.machine.global`")
            && error.0.contains("machine<initialization-outcome>"),
        "unexpected error: {error}"
    );
}
