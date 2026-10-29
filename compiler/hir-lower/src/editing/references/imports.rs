use super::*;
use crate::imports::{ImportedTargetBinding, ResolvedImportExposure};

mod linking;
mod reexports;

struct ResolvedReferenceTarget {
    target: Target,
    routes: Vec<hir::ReexportRouteV1>,
}

struct ResolvedReferenceImport {
    targets: Vec<ResolvedReferenceTarget>,
    alias: Option<hir::EditingImportAliasId>,
    name: Option<hir::SemanticSourceLocation>,
}

impl ReferenceBuilder {
    pub(super) fn imports(
        &mut self,
        lowerer: &mut Lowerer,
        check: &Rc<dyn Fn() -> bool>,
    ) -> Result<(), EditingCancelled> {
        for file in lowerer
            .imports
            .files
            .values()
            .chain(lowerer.imports.block_declarations.values())
        {
            for import in &file.unavailable {
                if check() {
                    return Err(EditingCancelled);
                }
                let binding = lowerer.imports.binding(import.binding);
                self.unavailable.push(hir::SemanticSourceLocation {
                    source: binding.source.clone(),
                    span: binding.span,
                });
            }
        }
        let declarations = lowerer
            .imports
            .files
            .values()
            .chain(lowerer.imports.block_declarations.values())
            .flat_map(|file| file.exact.iter().cloned())
            .collect::<Vec<_>>();
        let stars = lowerer
            .imports
            .files
            .values()
            .chain(lowerer.imports.block_declarations.values())
            .flat_map(|file| file.stars.iter().cloned())
            .collect::<Vec<_>>();
        let mut imports = HashMap::new();
        for import in declarations {
            if check() {
                return Err(EditingCancelled);
            }
            let origin = hir::SemanticSourceLocation {
                source: import.origin.source.clone(),
                span: import.origin.span,
            };
            let name_origin = hir::SemanticSourceLocation {
                span: import.target_span,
                ..origin.clone()
            };
            let mut complete = true;
            let mut targets = Vec::new();
            for binding in import.targets.iter() {
                if check() {
                    return Err(EditingCancelled);
                }
                if let Some(target) = self.import_target(lowerer, binding) {
                    self.record(target.target, name_origin.clone(), false);
                    self.export_reference(target.target, &name_origin, &target.routes, &[]);
                    targets.push(target);
                } else {
                    complete = false;
                }
            }
            if !complete {
                self.unavailable.push(name_origin.clone());
            }
            let alias = import.alias.filter(|_| complete).map(|name| {
                let alias_origin = hir::SemanticSourceLocation {
                    span: name.span,
                    ..origin.clone()
                };
                let mut represented = targets
                    .iter()
                    .map(|target| target.target)
                    .collect::<Vec<_>>();
                for target in &targets {
                    for route in &target.routes {
                        represented.extend(
                            route
                                .hops()
                                .iter()
                                .map(|hop| Target::ExportBinding(hop.binding())),
                        );
                    }
                }
                let id = self.aliases.alloc(hir::EditingImportAlias {
                    name: name.text,
                    origin: alias_origin.clone(),
                    targets: hir::NonEmptyVec::from_vec(represented)
                        .expect("a complete import retains its targets"),
                    reexported: import.exposure == ResolvedImportExposure::PublicReexport,
                    exports: Vec::new(),
                });
                self.record(Target::ImportAlias(id), alias_origin, true);
                id
            });
            imports.insert(
                origin,
                ResolvedReferenceImport {
                    targets,
                    alias,
                    name: Some(name_origin),
                },
            );
        }
        for import in stars {
            if check() {
                return Err(EditingCancelled);
            }
            let origin = hir::SemanticSourceLocation {
                source: import.origin.source,
                span: import.origin.span,
            };
            let mut targets = Vec::new();
            for binding in import
                .snapshot
                .values()
                .flat_map(|bindings| bindings.iter())
            {
                if check() {
                    return Err(EditingCancelled);
                }
                if let Some(target) = self.import_target(lowerer, binding) {
                    targets.push(target);
                } else {
                    self.unavailable.push(origin.clone());
                }
            }
            imports.insert(
                origin,
                ResolvedReferenceImport {
                    targets,
                    alias: None,
                    name: None,
                },
            );
        }
        self.reexports(lowerer, &imports, check)?;
        self.link_import_uses(lowerer, &imports, check)
    }

    fn import_target(
        &mut self,
        lowerer: &mut Lowerer,
        binding: &ImportedTargetBinding,
    ) -> Option<ResolvedReferenceTarget> {
        Some(ResolvedReferenceTarget {
            target: self.identities.import_target(lowerer, binding)?,
            routes: binding.direct_binding().map_or_else(Vec::new, |binding| {
                binding
                    .sources()
                    .map(|source| source.route().clone())
                    .collect()
            }),
        })
    }

    fn export_reference(
        &mut self,
        target: Target,
        origin: &hir::SemanticSourceLocation,
        routes: &[hir::ReexportRouteV1],
        aliases: &[hir::SemanticSourceLocation],
    ) {
        if routes.is_empty() {
            return;
        }
        if let Some(previous) = self
            .export_references
            .iter_mut()
            .find(|reference| reference.target == target && reference.origin == *origin)
        {
            for route in routes {
                if !previous.routes.contains(route) {
                    previous.routes.push(route.clone());
                }
            }
            for alias in aliases {
                if !previous.aliases.contains(alias) {
                    previous.aliases.push(alias.clone());
                }
            }
        } else {
            self.export_references.push(hir::EditingExportReference {
                target,
                origin: origin.clone(),
                routes: routes.to_vec(),
                aliases: aliases.to_vec(),
            });
        }
    }
}
