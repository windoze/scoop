use super::*;
use crate::{Lowerer, namespace::TopLevelLookupLayer};

enum SelectorResult {
    Targets(Vec<CurrentUnitBindingId>),
    Namespaces(Vec<ResolvedNamespace>),
}

impl CurrentUnitImports {
    fn selector(
        &self,
        lowerer: &Lowerer,
        path: &ast::QualifiedNameSyntax,
        star: bool,
    ) -> Result<SelectorResult, &'static str> {
        let segments = path.segments().cloned().collect::<Vec<_>>();
        let (package, prefix) = lowerer
            .top_level_namespaces
            .longest_package_prefix(&segments);
        if prefix == segments.len() {
            return if star {
                Ok(SelectorResult::Namespaces(vec![
                    ResolvedNamespace::Package(package),
                ]))
            } else {
                Err("exact import requires an importable binding, not a package namespace")
            };
        }
        let mut namespaces = vec![ResolvedNamespace::Package(package)];
        for (offset, segment) in segments[prefix..].iter().enumerate() {
            let targets = self.canonicalize(namespaces.iter().flat_map(|namespace| {
                self.namespaces
                    .get(namespace)
                    .and_then(|members| members.get(&segment.text))
                    .into_iter()
                    .flatten()
                    .copied()
            }));
            if targets.is_empty() {
                return Err("import target is not available in the current compilation unit");
            }
            let final_segment = prefix + offset + 1 == segments.len();
            if final_segment && !star {
                return Ok(SelectorResult::Targets(targets));
            }
            namespaces.clear();
            for target in targets {
                if let Some(namespace) = self.static_targets.get(&target) {
                    let namespace = ResolvedNamespace::Static(*namespace);
                    if !namespaces.contains(&namespace) {
                        namespaces.push(namespace);
                    }
                }
            }
            if namespaces.is_empty() {
                return Err("import target is not available in the current compilation unit");
            }
        }
        Ok(SelectorResult::Namespaces(namespaces))
    }

    fn imported(&self, lowerer: &Lowerer, id: CurrentUnitBindingId) -> Option<ImportedBinding> {
        let binding = self.binding(id);
        lowerer
            .access_domain_allows(&binding.access.0, None)
            .then(|| ImportedBinding {
                binding: id,
                sources: ast::NonEmptyVec::new(
                    CurrentUnitImportWitness {
                        source_binding: id,
                        site: lowerer.visibility_file(lowerer.current_file),
                        access: binding.access.clone(),
                    },
                    Vec::new(),
                ),
            })
    }

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
        let mut frozen = FrozenFileImports::default();
        let source_handle = match lowerer.visibility_file(lowerer.current_file).source {
            hir::VisibilitySource::CurrentUnit(source) => source,
            hir::VisibilitySource::ExistingM22Core { .. } => {
                unreachable!("core never enters import resolution")
            }
        };
        for import in &source.imports {
            match import {
                ast::ImportSyntax::Exact {
                    exposure,
                    selector,
                    alias,
                    span,
                    ..
                } => {
                    let targets = match self.selector(lowerer, selector, false) {
                        Ok(SelectorResult::Targets(targets)) => targets,
                        Ok(SelectorResult::Namespaces(_)) => {
                            unreachable!("exact selectors end in bindings")
                        }
                        Err(message) => {
                            lowerer.error(selector.span, message.to_string());
                            continue;
                        }
                    };
                    if !Self::public_gate(lowerer, *exposure) {
                        continue;
                    }
                    let mut accessible = targets
                        .iter()
                        .filter_map(|target| self.imported(lowerer, *target));
                    let Some(first) = accessible.next() else {
                        let last = selector.segments().last().expect("selector is non-empty");
                        let mut error = ast::Diagnostic::at_file(
                            lowerer.current_file,
                            last.span,
                            "import target is not accessible from this source location",
                        );
                        for target in targets {
                            let declaration = self.binding(target);
                            error.notes.push(ast::DiagnosticNote::at(
                                declaration.file,
                                declaration.span,
                                "declaration is outside this file's permitted access domain",
                            ));
                        }
                        lowerer.diagnostics.push(error);
                        continue;
                    };
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
                        targets: ast::NonEmptyVec::new(first, accessible.collect()),
                        origin: ImportSyntaxOrigin {
                            source: source_handle,
                            span: *span,
                        },
                    });
                }
                ast::ImportSyntax::Star {
                    exposure,
                    namespace,
                    span,
                    ..
                } => {
                    let namespaces = match self.selector(lowerer, namespace, true) {
                        Ok(SelectorResult::Namespaces(namespaces)) => namespaces,
                        Ok(SelectorResult::Targets(_)) => {
                            unreachable!("star selectors end in namespaces")
                        }
                        Err(message) => {
                            lowerer.error(namespace.span, message.to_string());
                            continue;
                        }
                    };
                    if !Self::public_gate(lowerer, *exposure) {
                        continue;
                    }
                    let candidates = namespaces
                        .iter()
                        .copied()
                        .filter(|namespace| self.namespace_is_accessible(lowerer, *namespace))
                        .collect::<Vec<_>>();
                    let selected = match (candidates.as_slice(), namespaces.as_slice()) {
                        ([namespace], _) | ([], [namespace]) => *namespace,
                        _ => {
                            lowerer.error(
                                namespace.span,
                                "import namespace is ambiguous in the current compilation unit"
                                    .to_string(),
                            );
                            continue;
                        }
                    };
                    let snapshot = self
                        .namespaces
                        .get(&selected)
                        .into_iter()
                        .flat_map(|members| members.iter())
                        .filter_map(|(name, targets)| {
                            let mut bindings = self
                                .canonicalize(targets.iter().copied())
                                .into_iter()
                                .filter_map(|id| self.imported(lowerer, id));
                            let first = bindings.next()?;
                            Some((
                                name.clone(),
                                ast::NonEmptyVec::new(first, bindings.collect()),
                            ))
                        })
                        .collect();
                    frozen.stars.push(ResolvedStarImport {
                        namespace: selected,
                        snapshot,
                        origin: ImportSyntaxOrigin {
                            source: source_handle,
                            span: *span,
                        },
                    });
                }
            }
        }
        frozen
    }

    fn namespace_is_accessible(&self, lowerer: &Lowerer, namespace: ResolvedNamespace) -> bool {
        let ResolvedNamespace::Static(namespace) = namespace else {
            return true;
        };
        self.static_targets.iter().any(|(binding, candidate)| {
            *candidate == namespace
                && lowerer.access_domain_allows(&self.binding(*binding).access.0, None)
        })
    }

    pub(super) fn validate_frozen_scopes(&self, lowerer: &Lowerer) {
        for (file, imports) in self.files.iter().enumerate() {
            let TopLevelLookupLayer::CurrentPackage(package) =
                lowerer.top_level_namespaces.source_namespace(file)
            else {
                assert!(imports.exact.is_empty() && imports.stars.is_empty());
                continue;
            };
            let site = lowerer.visibility_file(file);
            let check_target = |target: &ImportedBinding| {
                let binding = self.binding(target.binding);
                assert!(
                    !matches!(
                        binding.target,
                        CurrentUnitTarget::SourceProperty(_) | CurrentUnitTarget::SourceVariant(_)
                    ),
                    "a frozen body import has a complete declaration target"
                );
                for source in target.sources.iter() {
                    assert_eq!(source.source_binding, target.binding);
                    assert_eq!(source.site, site);
                    assert_eq!(source.access, binding.access);
                }
            };
            for import in &imports.exact {
                assert_eq!(
                    site.source,
                    hir::VisibilitySource::CurrentUnit(import.origin.source)
                );
                assert!(import.origin.span.start <= import.origin.span.end);
                for target in import.targets.iter() {
                    check_target(target);
                }
                let layers = self.layers(file, package, &import.local_name);
                assert_eq!(layers[0].kind, ImportLookupLayer::Exact);
                assert!(!layers[0].bindings.is_empty());
            }
            for import in &imports.stars {
                assert_eq!(
                    site.source,
                    hir::VisibilitySource::CurrentUnit(import.origin.source)
                );
                assert!(import.origin.span.start <= import.origin.span.end);
                let namespace = self.namespaces.get(&import.namespace);
                for (name, targets) in &import.snapshot {
                    for target in targets.iter() {
                        check_target(target);
                        assert!(
                            namespace
                                .and_then(|members| members.get(name))
                                .is_some_and(|members| members.contains(&target.binding))
                        );
                    }
                }
            }
        }
    }
}
