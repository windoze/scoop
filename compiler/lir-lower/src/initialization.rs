use super::*;

pub(super) fn lower_initialization_units(
    module: &mir::Module,
    globals: &HashMap<mir::GlobalId, StorageGlobal>,
    functions: &HashMap<mir::FunctionId, lir::LocalFunctionRef>,
) -> Arena<lir::InitializationUnit> {
    let mut units = Arena::new();
    for (source_id, source) in module.initialization_units.iter() {
        let kind = match source.kind {
            mir::InitializationUnitKind::EagerTopLevel { storage } => {
                lir::InitializationUnitKind::EagerTopLevel {
                    storage: local_global(globals, storage),
                }
            }
            mir::InitializationUnitKind::LazySingleton { published_root, .. } => {
                lir::InitializationUnitKind::LazySingleton {
                    published_root: local_global(
                        globals,
                        module.singleton_published_roots[published_root].global,
                    ),
                }
            }
        };
        let failure = module.initialization_failure_roots[source.failure_root].global;
        let id = units.alloc(lir::InitializationUnit {
            stable_key: source.stable_key.clone(),
            display_name: source.display_name.clone(),
            schedule: match source.schedule {
                mir::InitializationSchedule::EagerStartup => {
                    lir::InitializationSchedule::EagerStartup
                }
                mir::InitializationSchedule::LazyAccess => lir::InitializationSchedule::LazyAccess,
            },
            kind,
            failure_root: local_global(globals, failure),
            initializer: managed_function(functions, source.initializer),
            ensure: managed_function(functions, source.ensure),
            dependencies: source
                .dependencies
                .iter()
                .map(|dependency| lir::InitializationUnitId::from_raw(dependency.into_raw()))
                .collect(),
        });
        assert_eq!(source_id.into_raw(), id.into_raw());
    }
    units
}

fn local_global(
    globals: &HashMap<mir::GlobalId, StorageGlobal>,
    global: mir::GlobalId,
) -> lir::GlobalId {
    match globals[&global] {
        StorageGlobal::Local(global) => global,
        StorageGlobal::Native(_) => {
            unreachable!("initialization units can reference only compiler-managed storage")
        }
    }
}

fn managed_function(
    functions: &HashMap<mir::FunctionId, lir::LocalFunctionRef>,
    function: mir::FunctionId,
) -> lir::ManagedLocalFunctionRef {
    match functions[&function] {
        lir::LocalFunctionRef::Managed(function) => function,
        lir::LocalFunctionRef::NoGc(_) => {
            unreachable!("initialization callables are always managed")
        }
    }
}
