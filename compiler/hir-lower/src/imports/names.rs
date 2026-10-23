use super::*;
use std::collections::BTreeSet;

impl CurrentUnitImports {
    pub(crate) fn lookup_names(&self, file: usize, package: Option<PackageId>) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        if let Some(package) = package {
            names.extend(
                self.namespace_members(ResolvedNamespace::Package(package))
                    .map(|(name, _)| name.clone()),
            );
        }
        if let Some(imports) = self.files.get(&file) {
            names.extend(imports.exact.iter().map(|import| import.local_name.clone()));
            names.extend(imports.current_package_dependencies.keys().cloned());
            names.extend(
                imports
                    .stars
                    .iter()
                    .flat_map(|import| import.snapshot.keys().cloned()),
            );
        }
        names.extend(
            self.prelude_dependencies
                .values()
                .flat_map(|bindings| bindings.keys().cloned()),
        );
        names
    }
}
