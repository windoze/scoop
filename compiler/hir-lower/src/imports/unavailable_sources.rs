//! Known declarations remain lookup barriers when their package is incomplete.

use super::*;

impl CurrentUnitImports {
    pub(super) fn unknown_package_bindings<'a>(
        &'a self,
        name: &str,
    ) -> impl Iterator<Item = CurrentUnitBindingId> + 'a {
        self.unknown_packages
            .get(name)
            .into_iter()
            .flatten()
            .copied()
    }

    pub(super) fn insert_unknown_package(&mut self, binding: CurrentUnitBinding) {
        let id = CurrentUnitBindingId(self.bindings.len());
        self.unknown_packages
            .entry(binding.name.clone())
            .or_default()
            .push(id);
        self.bindings.push(binding);
    }

    pub(super) fn namespace_bindings<'a>(
        &'a self,
        namespace: ResolvedNamespace,
        name: &'a str,
    ) -> impl Iterator<Item = CurrentUnitBindingId> + 'a {
        self.namespaces
            .get(&namespace)
            .and_then(|members| members.get(name))
            .into_iter()
            .flatten()
            .copied()
            .chain(
                matches!(namespace, ResolvedNamespace::Package(_))
                    .then(|| self.unknown_packages.get(name))
                    .flatten()
                    .into_iter()
                    .flatten()
                    .copied(),
            )
    }

    pub(super) fn namespace_members(
        &self,
        namespace: ResolvedNamespace,
    ) -> impl Iterator<Item = (&String, &Vec<CurrentUnitBindingId>)> {
        self.namespaces.get(&namespace).into_iter().flatten().chain(
            matches!(namespace, ResolvedNamespace::Package(_))
                .then_some(&self.unknown_packages)
                .into_iter()
                .flatten(),
        )
    }
}
