use std::collections::BTreeMap;

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::BindingNamespace;

use super::{NamespaceCursor, SelectorError, SelectorGroup, SelectorResult};
use crate::{
    Lowerer,
    imports::{
        CurrentUnitBindingId, CurrentUnitImportWitness, CurrentUnitImports, ImportedTargetBinding,
        ResolvedNamespace,
    },
};

impl CurrentUnitImports {
    pub(super) fn cursor_group(
        &self,
        lowerer: &Lowerer,
        cursor: &NamespaceCursor<'_, '_>,
        name: &str,
    ) -> SelectorGroup {
        let mut group =
            self.cursor_group_for_namespace(lowerer, cursor, BindingNamespace::Type, name);
        let values =
            self.cursor_group_for_namespace(lowerer, cursor, BindingNamespace::Value, name);
        group.current.extend(values.current);
        group.current = self.canonicalize(group.current);
        group.direct.extend(values.direct);
        group.inaccessible.extend(values.inaccessible);
        group.inaccessible = self.canonicalize(group.inaccessible);
        group
    }

    pub(super) fn cursor_group_for_namespace(
        &self,
        lowerer: &Lowerer,
        cursor: &NamespaceCursor<'_, '_>,
        namespace: BindingNamespace,
        name: &str,
    ) -> SelectorGroup {
        let mut group = SelectorGroup::default();
        match cursor {
            NamespaceCursor::Stored { identity, groups } => {
                let current = identity.current();
                if current.is_some()
                    || matches!(
                        identity,
                        crate::imports::ResolvedImportNamespace::DirectPackage(_)
                    )
                {
                    self.extend_current_group(lowerer, &mut group, current, namespace, name);
                }
                group.direct.extend(
                    groups
                        .iter()
                        .filter(|candidate| {
                            candidate.namespace() == namespace && candidate.name().as_str() == name
                        })
                        .flat_map(|candidate| candidate.targets().iter().cloned()),
                );
            }
            NamespaceCursor::Current(current) => {
                self.extend_current_group(lowerer, &mut group, Some(*current), namespace, name);
            }
            NamespaceCursor::Direct(direct) => {
                let bindings = match direct {
                    hir::DirectNamespaceView::Package(package) => {
                        self.extend_current_group(lowerer, &mut group, None, namespace, name);
                        package.binding_group_filtered(namespace, name, |provider| {
                            lowerer.dependency_in_source_scope(provider)
                        })
                    }
                    hir::DirectNamespaceView::Static(namespace_view) => {
                        namespace_view.binding_group(namespace, name)
                    }
                };
                Self::extend_direct_group(&mut group, bindings);
            }
            NamespaceCursor::SplitPackage { current, direct } => {
                self.extend_current_group(
                    lowerer,
                    &mut group,
                    Some(ResolvedNamespace::Package(*current)),
                    namespace,
                    name,
                );
                Self::extend_direct_group(
                    &mut group,
                    direct.binding_group_filtered(namespace, name, |provider| {
                        lowerer.dependency_in_source_scope(provider)
                    }),
                );
            }
        }
        group.direct = self.canonicalize_dependencies(group.direct);
        group
    }

    fn extend_current_group(
        &self,
        lowerer: &Lowerer,
        group: &mut SelectorGroup,
        namespace: Option<ResolvedNamespace>,
        binding_namespace: BindingNamespace,
        name: &str,
    ) {
        let bindings = match namespace {
            Some(namespace) => self.namespace_bindings(namespace, name).collect::<Vec<_>>(),
            None => self.unknown_package_bindings(name).collect(),
        };
        for binding in self.canonicalize(bindings.into_iter().filter(|binding| {
            self.binding(*binding)
                .target
                .occupies_namespace(binding_namespace)
        })) {
            if !lowerer.declaration_source_in_scope(self.binding(binding).file) {
                continue;
            }
            if lowerer.access_domain_allows(&self.binding(binding).access.0) {
                group.current.push(binding);
            } else {
                group.inaccessible.push(binding);
            }
        }
    }

    fn extend_direct_group(
        group: &mut SelectorGroup,
        bindings: Option<hir::DirectPublicBindingGroup<'_, '_>>,
    ) {
        if let Some(bindings) = bindings {
            group.direct.extend(bindings.targets().cloned());
        }
    }

    pub(super) fn materialize_non_empty_group(
        &self,
        lowerer: &Lowerer,
        group: SelectorGroup,
    ) -> Result<SelectorResult, SelectorError> {
        let inaccessible = group.inaccessible;
        if has_distinct_target_conflict(&group.direct) {
            return Err(SelectorError::AmbiguousBinding);
        }
        let targets = self.materialize_group(lowerer, group.current, group.direct);
        let mut targets = targets.into_iter();
        let Some(first) = targets.next() else {
            return Err(SelectorError::Missing { inaccessible });
        };
        Ok(SelectorResult::Targets(ast::NonEmptyVec::new(
            first,
            targets.collect(),
        )))
    }

    fn materialize_group(
        &self,
        lowerer: &Lowerer,
        current: Vec<CurrentUnitBindingId>,
        direct: Vec<hir::DirectImportedTargetBinding>,
    ) -> Vec<ImportedTargetBinding> {
        current
            .into_iter()
            .map(|binding| {
                let declaration = self.binding(binding);
                ImportedTargetBinding::current(
                    binding,
                    ast::NonEmptyVec::new(
                        CurrentUnitImportWitness {
                            source_binding: binding,
                            site: lowerer.visibility_file(lowerer.current_file),
                            access: declaration.access.clone(),
                        },
                        Vec::new(),
                    ),
                )
            })
            .chain(
                direct
                    .into_iter()
                    .map(ImportedTargetBinding::DirectDependency),
            )
            .collect()
    }

    pub(super) fn cursor_snapshot(
        &self,
        lowerer: &Lowerer,
        cursor: &NamespaceCursor<'_, '_>,
    ) -> BTreeMap<String, ast::NonEmptyVec<ImportedTargetBinding>> {
        let mut names = BTreeMap::<String, Vec<ImportedTargetBinding>>::new();
        let current = match cursor {
            NamespaceCursor::Current(namespace) => Some(*namespace),
            NamespaceCursor::Stored { identity, .. } => identity.current(),
            NamespaceCursor::SplitPackage { current, .. } => {
                Some(ResolvedNamespace::Package(*current))
            }
            NamespaceCursor::Direct(_) => None,
        };
        let source_names = match current {
            Some(current) => self
                .namespace_members(current)
                .map(|(name, _)| name)
                .collect(),
            None if matches!(
                cursor,
                NamespaceCursor::Direct(hir::DirectNamespaceView::Package(_))
                    | NamespaceCursor::Stored {
                        identity: crate::imports::ResolvedImportNamespace::DirectPackage(_),
                        ..
                    }
            ) =>
            {
                self.unknown_packages.keys().collect()
            }
            None => std::collections::BTreeSet::new(),
        };
        for name in source_names {
            let mut group = SelectorGroup::default();
            self.extend_current_group(lowerer, &mut group, current, BindingNamespace::Type, name);
            self.extend_current_group(lowerer, &mut group, current, BindingNamespace::Value, name);
            group.current = self.canonicalize(group.current);
            names
                .entry(name.clone())
                .or_default()
                .extend(self.materialize_group(lowerer, group.current, Vec::new()));
        }

        let direct = match cursor {
            NamespaceCursor::Stored { groups, .. } => Some(groups.clone()),
            NamespaceCursor::Direct(hir::DirectNamespaceView::Package(package)) => Some(
                package.snapshot_filtered(|provider| lowerer.dependency_in_source_scope(provider)),
            ),
            NamespaceCursor::Direct(hir::DirectNamespaceView::Static(namespace)) => {
                Some(namespace.snapshot())
            }
            NamespaceCursor::SplitPackage { direct, .. } => Some(
                direct.snapshot_filtered(|provider| lowerer.dependency_in_source_scope(provider)),
            ),
            NamespaceCursor::Current(_) => None,
        };
        for group in direct.into_iter().flatten() {
            names
                .entry(group.name().as_str().to_owned())
                .or_default()
                .extend(
                    group
                        .targets()
                        .iter()
                        .cloned()
                        .map(ImportedTargetBinding::DirectDependency),
                );
        }

        names
            .into_iter()
            .filter_map(|(name, targets)| {
                let mut targets = targets.into_iter();
                let first = targets.next()?;
                Some((name, ast::NonEmptyVec::new(first, targets.collect())))
            })
            .collect()
    }
}

fn has_distinct_target_conflict(bindings: &[hir::DirectImportedTargetBinding]) -> bool {
    bindings.iter().enumerate().any(|(index, binding)| {
        bindings[..index].iter().any(|previous| {
            previous.binding_target().namespace() == binding.binding_target().namespace()
                && previous.binding_target() != binding.binding_target()
                && previous
                    .conflict_key()
                    .conflicts_with(binding.conflict_key())
        })
    })
}
