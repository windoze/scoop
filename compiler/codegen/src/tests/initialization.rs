use super::*;

#[test]
fn emits_typed_initialization_descriptors_in_stable_key_order() {
    let mut module = values_module();
    let storage = module.globals.alloc(Global {
        symbol: "scoop.init.storage".to_string(),
        address_kind: PointerKind::Raw,
        scan: RefScan::None,
        init: GlobalInit::Storage {
            ty: LirType::I64,
            initializer: scoop_lir::ConstantValue::Zero,
            thread_local: false,
        },
    });
    let failure = module.globals.alloc(Global {
        symbol: "scoop.init.failure".to_string(),
        address_kind: PointerKind::Raw,
        scan: RefScan::References(vec![0]),
        init: GlobalInit::Storage {
            ty: MANAGED_PTR,
            initializer: scoop_lir::ConstantValue::Zero,
            thread_local: false,
        },
    });
    let second_storage = module.globals.alloc(Global {
        symbol: "scoop.init.storage.2".to_string(),
        address_kind: PointerKind::Raw,
        scan: RefScan::None,
        init: GlobalInit::Storage {
            ty: LirType::I64,
            initializer: scoop_lir::ConstantValue::Zero,
            thread_local: false,
        },
    });
    let second_failure = module.globals.alloc(Global {
        symbol: "scoop.init.failure.2".to_string(),
        address_kind: PointerKind::Raw,
        scan: RefScan::References(vec![0]),
        init: GlobalInit::Storage {
            ty: MANAGED_PTR,
            initializer: scoop_lir::ConstantValue::Zero,
            thread_local: false,
        },
    });
    let mut functions = scoop_lir::LocalFunctionIdentities::default();
    let entry = functions.alloc_managed();
    module
        .initialization_units
        .alloc(scoop_lir::InitializationUnit {
            stable_key: "top-level:zed".to_string(),
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
            stable_key: "top-level:alpha".to_string(),
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
    assert!(ir.contains("{ i64 0, ptr @scoop.init.key.0"));
    assert!(ir.contains("{ i64 1, ptr @scoop.init.key.1"));
    let table = ir
        .lines()
        .find(|line| line.starts_with("@scoop_image_initialization_units ="))
        .expect("initialization descriptor table");
    let alpha = table.find("@scoop.init.key.1").expect("alpha key in table");
    let zed = table.find("@scoop.init.key.0").expect("zed key in table");
    assert!(alpha < zed, "descriptor table must be sorted by stable key");
    assert!(ir.contains("@scoop_image_initialization_unit_count = constant i64 2"));
}
