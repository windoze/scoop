use std::collections::BTreeMap;

use scoop_identity::{BindingNamespace, BindingTarget, CanonicalIdentifier, PackagePath};

use super::selector::SelectorResult;
use super::*;
use crate::{Lowerer, namespace::TopLevelLookupLayer};

impl CurrentUnitImports {
    #[cfg(test)]
    pub(super) fn resolve_file(
        &self,
        lowerer: &mut Lowerer,
        source: &ast::SourceFile,
    ) -> FrozenFileImports {
        self.resolve_file_with_world(lowerer, source, None)
    }

    fn resolve_exposure<'a>(
        lowerer: &mut Lowerer,
        exposure: ast::ImportExposureSyntax,
        selector_span: ast::Span,
        allow_empty_direct_namespace: bool,
        targets: impl IntoIterator<Item = &'a ImportedTargetBinding>,
    ) -> Option<ResolvedImportExposure> {
        match exposure {
            ast::ImportExposureSyntax::Local => Some(ResolvedImportExposure::Local),
            ast::ImportExposureSyntax::PublicReexport { .. } => {
                let mut saw_target = false;
                let all_direct = targets.into_iter().all(|target| {
                    saw_target = true;
                    matches!(target, ImportedTargetBinding::DirectDependency(_))
                });
                if all_direct && (saw_target || allow_empty_direct_namespace) {
                    Some(ResolvedImportExposure::PublicReexport)
                } else {
                    lowerer.error(
                        selector_span,
                        "public import requires a direct dependency target".to_string(),
                    );
                    None
                }
            }
        }
    }

    pub(super) fn resolve_file_with_world(
        &self,
        lowerer: &mut Lowerer,
        source: &ast::SourceFile,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
    ) -> FrozenFileImports {
        let diagnostics_before = lowerer.diagnostics.len();
        let mut frozen = FrozenFileImports {
            current_package_dependencies: Self::current_package_dependency_snapshot(lowerer, world),
            ..FrozenFileImports::default()
        };
        let mut exact_dependency_slots = BTreeMap::new();
        debug_assert!(lowerer.source_is_current_cone(lowerer.current_file));
        let source_identity = lowerer.visibility_file(lowerer.current_file);
        for import in &source.imports {
            match import {
                ast::ImportSyntax::Exact {
                    exposure,
                    selector,
                    alias,
                    span,
                    ..
                } => {
                    let targets = match self.selector(lowerer, world, selector, false) {
                        Ok(SelectorResult::Targets(targets)) => targets,
                        Ok(SelectorResult::Namespace { .. }) => {
                            unreachable!("exact selectors end in bindings")
                        }
                        Err(error) if !error.inaccessible().is_empty() => {
                            if matches!(exposure, ast::ImportExposureSyntax::PublicReexport { .. })
                            {
                                let _ = Self::resolve_exposure(
                                    lowerer,
                                    *exposure,
                                    selector.span,
                                    false,
                                    std::iter::empty(),
                                );
                                continue;
                            }
                            let last = selector.segments().last().expect("selector is non-empty");
                            let mut diagnostic = ast::Diagnostic::at_file(
                                lowerer.current_file,
                                last.span,
                                "import target is not accessible from this source location",
                            );
                            for target in error.inaccessible() {
                                let declaration = self.binding(*target);
                                diagnostic.notes.push(ast::DiagnosticNote::at(
                                    declaration.file,
                                    declaration.span,
                                    "declaration is outside this file's permitted access domain",
                                ));
                            }
                            lowerer.diagnostics.push(diagnostic);
                            continue;
                        }
                        Err(error) => {
                            lowerer.error(selector.span, error.message().to_string());
                            continue;
                        }
                    };
                    let Some(exposure) = Self::resolve_exposure(
                        lowerer,
                        *exposure,
                        selector.span,
                        false,
                        targets.iter(),
                    ) else {
                        continue;
                    };
                    let local_name = alias
                        .as_ref()
                        .map(|alias| alias.name.text.clone())
                        .unwrap_or_else(|| {
                            selector
                                .segments()
                                .last()
                                .expect("selector is non-empty")
                                .text
                                .clone()
                        });
                    if !stage_exact_dependency_targets(
                        &mut exact_dependency_slots,
                        &local_name,
                        &targets,
                    ) {
                        lowerer.error(
                            selector.span,
                            "import target is ambiguous in the current compilation unit"
                                .to_string(),
                        );
                        continue;
                    }
                    frozen.exact.push(ResolvedExactImport {
                        exposure,
                        local_name,
                        source_role: if alias.is_some() {
                            scoop_identity::LocalBindingRole::AliasImport
                        } else {
                            scoop_identity::LocalBindingRole::ExactImport
                        },
                        targets,
                        origin: ImportSyntaxOrigin {
                            source: source_identity.clone(),
                            span: *span,
                        },
                    });
                }
                ast::ImportSyntax::Star {
                    exposure,
                    namespace,
                    star_span,
                    span,
                    ..
                } => {
                    let selector_span = ast::Span::new(namespace.span.start, star_span.end);
                    let (selected, snapshot) = match self.selector(lowerer, world, namespace, true)
                    {
                        Ok(SelectorResult::Namespace { identity, snapshot }) => {
                            (identity, snapshot)
                        }
                        Ok(SelectorResult::Targets(_)) => {
                            unreachable!("star selectors end in namespaces")
                        }
                        Err(error) => {
                            lowerer.error(selector_span, error.message().to_string());
                            continue;
                        }
                    };
                    let allow_empty_direct_namespace =
                        !matches!(selected, ResolvedImportNamespace::Current(_));
                    let Some(exposure) = Self::resolve_exposure(
                        lowerer,
                        *exposure,
                        selector_span,
                        allow_empty_direct_namespace,
                        snapshot.values().flat_map(ast::NonEmptyVec::iter),
                    ) else {
                        continue;
                    };
                    frozen.stars.push(ResolvedStarImport {
                        exposure,
                        namespace: selected,
                        snapshot,
                        origin: ImportSyntaxOrigin {
                            source: source_identity.clone(),
                            span: *span,
                        },
                    });
                }
            }
        }
        if lowerer.diagnostics.len() == diagnostics_before {
            frozen
        } else {
            FrozenFileImports {
                current_package_dependencies: frozen.current_package_dependencies,
                ..FrozenFileImports::default()
            }
        }
    }

    fn current_package_dependency_snapshot(
        lowerer: &Lowerer,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
    ) -> BTreeMap<String, Vec<hir::DirectImportedTargetBinding>> {
        let crate::namespace::TopLevelLookupLayer::CurrentPackage(package) = lowerer
            .top_level_namespaces
            .source_namespace(lowerer.current_file)
        else {
            return BTreeMap::new();
        };
        let path = PackagePath::from_segments(
            lowerer
                .top_level_namespaces
                .package_segments(package)
                .into_iter()
                .map(|segment| {
                    CanonicalIdentifier::new(segment)
                        .expect("source package segments are canonical identifiers")
                })
                .collect(),
        );
        let Some(package) = world.and_then(|world| world.direct_package(&path)) else {
            return BTreeMap::new();
        };
        let prelude = world
            .and_then(|world| world.direct_provider(scoop_identity::ConeIdentity::CORE))
            .map(|provider| provider.id());
        let mut snapshot = BTreeMap::<String, Vec<_>>::new();
        // The implicit prelude is a later lookup layer, even when its ordinary
        // provider contributes to the same package as the current source.
        for group in package.snapshot_filtered(|provider| Some(provider) != prelude) {
            snapshot
                .entry(group.name().as_str().to_owned())
                .or_default()
                .extend(group.targets().iter().cloned());
        }
        snapshot
    }

    pub(super) fn validate_frozen_scopes(&self, lowerer: &Lowerer) {
        let check_materialized = |binding: CurrentUnitBindingId| {
            assert!(
                !matches!(
                    self.binding(binding).target,
                    CurrentUnitTarget::SourceProperty(_) | CurrentUnitTarget::SourceVariant(_)
                ),
                "a body lookup scope has a complete declaration target"
            );
        };
        for bindings in self
            .namespaces
            .values()
            .flat_map(|members| members.values())
        {
            for binding in bindings {
                check_materialized(*binding);
            }
        }
        for bindings in self.diagnostic_suppressions.unmaterialized_values.values() {
            for binding in bindings {
                assert!(
                    matches!(
                        self.binding(*binding).target,
                        CurrentUnitTarget::SourceProperty(_) | CurrentUnitTarget::SourceVariant(_)
                    ),
                    "diagnostic suppression retains only an unmaterialized source origin"
                );
            }
        }

        for (file, imports) in self.files.iter().enumerate() {
            let TopLevelLookupLayer::CurrentPackage(package) =
                lowerer.top_level_namespaces.source_namespace(file)
            else {
                assert!(imports.exact.is_empty() && imports.stars.is_empty());
                continue;
            };
            let site = lowerer.visibility_file(file);
            let check_target = |target: &ImportedTargetBinding| match target {
                ImportedTargetBinding::CurrentCone { binding, sources } => {
                    let declaration = self.binding(*binding);
                    check_materialized(*binding);
                    for source in sources.iter() {
                        assert_eq!(source.source_binding, *binding);
                        assert_eq!(source.site, site);
                        assert_eq!(source.access, declaration.access);
                    }
                }
                ImportedTargetBinding::DirectDependency(binding) => {
                    assert_ne!(binding.source_count(), 0);
                }
            };
            for import in &imports.exact {
                assert_eq!(site, import.origin.source);
                assert!(import.origin.span.start <= import.origin.span.end);
                for target in import.targets.iter() {
                    check_target(target);
                    if import.exposure == ResolvedImportExposure::PublicReexport {
                        assert!(matches!(target, ImportedTargetBinding::DirectDependency(_)));
                    }
                }
                let layers = self.layers(file, package, &import.local_name);
                assert_eq!(layers[0].kind, ImportLookupLayer::Exact);
                assert!(
                    !layers[0].bindings.is_empty() || !layers[0].dependency_bindings.is_empty()
                );
            }
            for import in &imports.stars {
                assert_eq!(site, import.origin.source);
                assert!(import.origin.span.start <= import.origin.span.end);
                let namespace = import
                    .namespace
                    .current()
                    .and_then(|namespace| self.namespaces.get(&namespace));
                for (name, targets) in &import.snapshot {
                    for target in targets.iter() {
                        check_target(target);
                        if import.exposure == ResolvedImportExposure::PublicReexport {
                            assert!(matches!(target, ImportedTargetBinding::DirectDependency(_)));
                        }
                        if let Some(binding) = target.current_binding() {
                            assert!(
                                namespace
                                    .and_then(|members| members.get(name))
                                    .is_some_and(|members| members.contains(&binding))
                            );
                        }
                    }
                }
            }
        }
    }
}

fn stage_exact_dependency_targets(
    slots: &mut BTreeMap<
        (String, BindingNamespace),
        Vec<(BindingTarget, hir::ImportedBindingConflictKey)>,
    >,
    local_name: &str,
    targets: &ast::NonEmptyVec<ImportedTargetBinding>,
) -> bool {
    let direct = targets
        .iter()
        .filter_map(ImportedTargetBinding::direct_binding)
        .collect::<Vec<_>>();
    for (index, target) in direct.iter().enumerate() {
        let slot = (local_name.to_owned(), target.binding_target().namespace());
        let conflicts_with = |existing: &(BindingTarget, hir::ImportedBindingConflictKey)| {
            existing.0 != target.binding_target()
                && existing.1.conflicts_with(target.conflict_key())
        };
        if slots
            .get(&slot)
            .is_some_and(|existing| existing.iter().any(conflicts_with))
            || direct[..index].iter().any(|existing| {
                existing.binding_target().namespace() == target.binding_target().namespace()
                    && existing.binding_target() != target.binding_target()
                    && existing
                        .conflict_key()
                        .conflicts_with(target.conflict_key())
            })
        {
            return false;
        }
    }
    for target in direct {
        let slot = (local_name.to_owned(), target.binding_target().namespace());
        let occupied = slots.entry(slot).or_default();
        if !occupied
            .iter()
            .any(|(existing, _)| *existing == target.binding_target())
        {
            occupied.push((target.binding_target(), target.conflict_key().clone()));
        }
    }
    true
}
