use super::selector::SelectorResult;
use super::*;
use crate::SourceKind;
use crate::{Lowerer, namespace::TopLevelLookupLayer};

impl CurrentUnitImports {
    fn public_gate(lowerer: &mut Lowerer, exposure: ast::ImportExposureSyntax) -> bool {
        match exposure {
            ast::ImportExposureSyntax::Local => true,
            ast::ImportExposureSyntax::PublicReexport {
                public_keyword_span,
            } => {
                lowerer.error(
                    public_keyword_span,
                    "public import requires a direct dependency target".to_string(),
                );
                false
            }
        }
    }

    pub(super) fn resolve_file(
        &self,
        lowerer: &mut Lowerer,
        source: &ast::SourceFile,
    ) -> FrozenFileImports {
        self.resolve_file_with_world(lowerer, source, None)
    }

    pub(super) fn resolve_file_with_world(
        &self,
        lowerer: &mut Lowerer,
        source: &ast::SourceFile,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
    ) -> FrozenFileImports {
        let mut frozen = FrozenFileImports::default();
        debug_assert_eq!(
            lowerer.intrinsic_sources[lowerer.current_file].kind,
            SourceKind::CurrentUnit
        );
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
                            if !Self::public_gate(lowerer, *exposure) {
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
                    if !Self::public_gate(lowerer, *exposure) {
                        continue;
                    }
                    frozen.exact.push(ResolvedExactImport {
                        local_name: alias
                            .as_ref()
                            .map(|alias| alias.name.text.clone())
                            .unwrap_or_else(|| {
                                selector
                                    .segments()
                                    .last()
                                    .expect("selector is non-empty")
                                    .text
                                    .clone()
                            }),
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
                    if !Self::public_gate(lowerer, *exposure) {
                        continue;
                    }
                    frozen.stars.push(ResolvedStarImport {
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
        frozen
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
