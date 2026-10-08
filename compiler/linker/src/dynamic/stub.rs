use super::*;
use scoop_toolchain::PreviousExport;

pub(super) fn entries(
    exports: BTreeMap<String, NativeExport>,
    previous: &BTreeMap<String, PreviousExport>,
    reexports: Vec<String>,
) -> (BTreeMap<String, DynamicExport>, Vec<LoadDependency>) {
    let mut exports: BTreeMap<_, _> = exports
        .into_iter()
        .map(|(name, interface)| {
            (
                name,
                DynamicExport::Symbol {
                    interface,
                    storage: ExportStorage::InterfaceOnly,
                },
            )
        })
        .collect();
    let mut dependencies: Vec<_> = reexports
        .into_iter()
        .map(|name| LoadDependency {
            name,
            reexport: true,
            compatibility_version: 0,
        })
        .collect();
    for (symbol, previous) in previous {
        let dependency = if let Some(index) = dependencies
            .iter()
            .position(|dependency| dependency.name == previous.install_name)
        {
            dependencies[index].compatibility_version = dependencies[index]
                .compatibility_version
                .max(previous.compatibility_version);
            index
        } else {
            let index = dependencies.len();
            dependencies.push(LoadDependency {
                name: previous.install_name.clone(),
                reexport: false,
                compatibility_version: previous.compatibility_version,
            });
            index
        };
        exports.insert(
            symbol.clone(),
            DynamicExport::Previous {
                dependency,
                interface: previous.interface,
            },
        );
    }
    (exports, dependencies)
}
