//! Exact-import binding resolution (DESIGN sections 2.2-2.3).
//!
//! After every declaration exists, each file's exact imports resolve
//! against the current Cone's declarations: the longest declared-package
//! prefix is taken first, remaining segments walk static nested nominal
//! owners, and the final segment binds every namespace it names
//! (overload sets contribute one target per function). Top-level
//! `private` declarations are invisible to imports from other files.

use std::collections::HashMap;

use super::*;
use hir::DeclaredVisibility;

/// One namespace hit, with the file that declared it (for private
/// visibility) already resolved.
struct Hit<T> {
    entity: T,
    declaring_file: usize,
    private: bool,
}

/// One package-scoped namespace hit bucket.
#[derive(Default)]
struct NamespaceHits {
    functions: Vec<Hit<hir::FunctionId>>,
    structs: Vec<Hit<hir::StructId>>,
    enums: Vec<Hit<hir::EnumId>>,
    classes: Vec<Hit<hir::ClassId>>,
    interfaces: Vec<Hit<hir::InterfaceId>>,
    objects: Vec<Hit<hir::ObjectId>>,
    type_aliases: Vec<Hit<hir::ExportTypeAliasId>>,
    properties: Vec<Hit<hir::PropertyId>>,
}

impl NamespaceHits {
    fn any_hits(&self) -> bool {
        !self.functions.is_empty()
            || !self.structs.is_empty()
            || !self.enums.is_empty()
            || !self.classes.is_empty()
            || !self.interfaces.is_empty()
            || !self.objects.is_empty()
            || !self.type_aliases.is_empty()
            || !self.properties.is_empty()
    }

    fn any_visible_from(&self, file: usize) -> bool {
        fn any_visible<T>(hits: &[Hit<T>], file: usize) -> bool {
            hits.iter()
                .any(|hit| !hit.private || hit.declaring_file == file)
        }
        any_visible(&self.functions, file)
            || any_visible(&self.structs, file)
            || any_visible(&self.enums, file)
            || any_visible(&self.classes, file)
            || any_visible(&self.interfaces, file)
            || any_visible(&self.objects, file)
            || any_visible(&self.type_aliases, file)
            || any_visible(&self.properties, file)
    }
}

/// (package id, top-level name) -> namespace hits.
struct PackageIndex {
    hits: HashMap<(hir::PackageId, String), NamespaceHits>,
}

impl Lowerer {
    /// Resolves every file's exact imports in deterministic file and
    /// declaration order; unresolved or invisible selectors are
    /// diagnosed once per import.
    pub(super) fn resolve_import_bindings(&mut self) {
        let index = self.build_package_index();
        for file in 0..self.file_imports.len() {
            for import_index in 0..self.file_imports[file].exact.len() {
                let (path, local_name, span) = {
                    let import = &self.file_imports[file].exact[import_index];
                    (
                        import.path.clone(),
                        import.local_name().to_owned(),
                        import.span,
                    )
                };
                match self.resolve_exact(&index, file, &path, &local_name, span) {
                    Ok(binding) => {
                        self.file_imports[file].exact[import_index].binding = Some(binding);
                    }
                    Err(message) => self.diagnostics.push(Diagnostic::at(span, message)),
                }
            }
        }
    }

    /// Indexes every declaration by (package, name). The declaring file
    /// comes from each declaration's origin; objects ride on their
    /// backing class and top-level properties on their backing global.
    fn build_package_index(&self) -> PackageIndex {
        let mut hits: HashMap<(hir::PackageId, String), NamespaceHits> = HashMap::new();
        let package_of = |file: u32| -> hir::PackageId { self.file_packages[file as usize] };
        let private_of =
            |declared: DeclaredVisibility| matches!(declared, DeclaredVisibility::Private);

        for (id, decl) in self.structs.iter() {
            let entry = hits
                .entry((package_of(decl.origin.file), decl.name.clone()))
                .or_default();
            entry.structs.push(Hit {
                entity: id,
                declaring_file: decl.origin.file as usize,
                private: private_of(decl.access.declared),
            });
        }
        for (id, decl) in self.enums.iter() {
            let entry = hits
                .entry((package_of(decl.origin.file), decl.name.clone()))
                .or_default();
            entry.enums.push(Hit {
                entity: id,
                declaring_file: decl.origin.file as usize,
                private: private_of(decl.access.declared),
            });
        }
        for (id, decl) in self.classes.iter() {
            let entry = hits
                .entry((package_of(decl.origin.file), decl.name.clone()))
                .or_default();
            entry.classes.push(Hit {
                entity: id,
                declaring_file: decl.origin.file as usize,
                private: private_of(decl.access.declared),
            });
        }
        for (id, decl) in self.interfaces.iter() {
            let entry = hits
                .entry((package_of(decl.origin.file), decl.name.clone()))
                .or_default();
            entry.interfaces.push(Hit {
                entity: id,
                declaring_file: decl.origin.file as usize,
                private: private_of(decl.access.declared),
            });
        }
        for (id, decl) in self.objects.iter() {
            // Objects declare no origin of their own; the backing class
            // is generated in the object's source file.
            let backing = &self.classes[decl.backing_class];
            let entry = hits
                .entry((package_of(backing.origin.file), decl.name.clone()))
                .or_default();
            entry.objects.push(Hit {
                entity: id,
                declaring_file: backing.origin.file as usize,
                private: private_of(decl.access.declared),
            });
        }
        for (id, function) in self.functions.iter() {
            // Member functions are reached through their owner, never
            // through a package-level import selector.
            if function.method.is_some() {
                continue;
            }
            let entry = hits
                .entry((package_of(function.origin.file), function.name.clone()))
                .or_default();
            entry.functions.push(Hit {
                entity: id,
                declaring_file: function.origin.file as usize,
                private: private_of(function.access.declared),
            });
        }
        for (id, alias) in self.type_aliases.iter() {
            let entry = hits
                .entry((package_of(alias.origin.file), alias.name.clone()))
                .or_default();
            entry.type_aliases.push(Hit {
                entity: id,
                declaring_file: alias.origin.file as usize,
                private: private_of(alias.access.declared),
            });
        }
        for (_global_id, global) in self.globals.iter() {
            // Top-level properties are backed by globals; the property
            // owns the source-level name.
            let property = &self.properties[global.property];
            let entry = hits
                .entry((package_of(global.origin.file), property.name.clone()))
                .or_default();
            entry.properties.push(Hit {
                entity: global.property,
                declaring_file: global.origin.file as usize,
                private: private_of(property.access.declared),
            });
        }
        PackageIndex { hits }
    }

    /// Resolves one selector: the longest declared-package prefix wins;
    /// remaining segments walk nested nominal owners; the final segment
    /// binds every visible namespace it names.
    fn resolve_exact(
        &self,
        index: &PackageIndex,
        file: usize,
        path: &[String],
        local_name: &str,
        witness: scoop_ast::Span,
    ) -> Result<hir::ImportedBinding, String> {
        let path_text = path.join(".");
        let mut existed_somewhere = false;
        // k = number of package-prefix segments; the selector needs at
        // least one segment for the target name, so k <= path.len() - 1.
        for k in (0..path.len()).rev() {
            let package = match self.package_by_segments(&path[..k]) {
                Some(package) => package,
                None => continue,
            };
            let rest = &path[k..];
            if rest.len() == 1 {
                match index.hits.get(&(package, rest[0].clone())) {
                    Some(hits) if hits.any_hits() => {
                        existed_somewhere = true;
                        if !hits.any_visible_from(file) {
                            continue;
                        }
                        let targets = self.single_segment_targets(hits, file, witness);
                        if targets.is_empty() {
                            continue;
                        }
                        return Ok(hir::ImportedBinding::of(local_name.to_owned(), targets));
                    }
                    _ => continue,
                }
            }
            // Multi-segment: the first segment names a top-level
            // nominal; intermediate segments and the final segment walk
            // nested nominal owners.
            match self.walk_nested(index, package, file, rest, local_name, witness) {
                Walked::Resolved(binding) => return Ok(binding),
                Walked::Existed => {
                    existed_somewhere = true;
                }
                Walked::NoMatch => {}
            }
        }
        if existed_somewhere {
            Err(format!(
                "import `{path_text}` names declarations that are not visible in this file"
            ))
        } else {
            Err(format!("unresolved import `{path_text}`"))
        }
    }

    fn single_segment_targets(
        &self,
        hits: &NamespaceHits,
        file: usize,
        witness: scoop_ast::Span,
    ) -> Vec<hir::ImportedTargetBinding> {
        let source = || hir::ImportBindingSource::CurrentCone { witness };
        fn visible<T>(hit: &Hit<T>, file: usize) -> bool {
            !hit.private || hit.declaring_file == file
        }
        let mut targets = Vec::new();
        for hit in &hits.functions {
            if visible(hit, file) {
                targets.push(hir::ImportedTargetBinding {
                    target: hir::ImportedTarget::Function {
                        function: hit.entity,
                    },
                    sources: vec![source()],
                });
            }
        }
        for hit in &hits.structs {
            if visible(hit, file) {
                targets.push(hir::ImportedTargetBinding {
                    target: hir::ImportedTarget::Struct {
                        declaration: hit.entity,
                    },
                    sources: vec![source()],
                });
            }
        }
        for hit in &hits.enums {
            if visible(hit, file) {
                targets.push(hir::ImportedTargetBinding {
                    target: hir::ImportedTarget::Enum {
                        declaration: hit.entity,
                    },
                    sources: vec![source()],
                });
            }
        }
        for hit in &hits.classes {
            if visible(hit, file) {
                targets.push(hir::ImportedTargetBinding {
                    target: hir::ImportedTarget::Class {
                        declaration: hit.entity,
                    },
                    sources: vec![source()],
                });
            }
        }
        for hit in &hits.interfaces {
            if visible(hit, file) {
                targets.push(hir::ImportedTargetBinding {
                    target: hir::ImportedTarget::Interface {
                        declaration: hit.entity,
                    },
                    sources: vec![source()],
                });
            }
        }
        for hit in &hits.objects {
            if visible(hit, file) {
                targets.push(hir::ImportedTargetBinding {
                    target: hir::ImportedTarget::Object {
                        declaration: hit.entity,
                    },
                    sources: vec![source()],
                });
            }
        }
        for hit in &hits.type_aliases {
            if visible(hit, file) {
                targets.push(hir::ImportedTargetBinding {
                    target: hir::ImportedTarget::TypeAlias { alias: hit.entity },
                    sources: vec![source()],
                });
            }
        }
        for hit in &hits.properties {
            if visible(hit, file) {
                targets.push(hir::ImportedTargetBinding {
                    target: hir::ImportedTarget::Property {
                        property: hit.entity,
                    },
                    sources: vec![source()],
                });
            }
        }
        targets
    }

    /// Walks `<top-level nominal>.<nested>...` from a package. The final
    /// segment must name a nested nominal; non-nominal members are not
    /// importable through an owner path.
    fn walk_nested(
        &self,
        index: &PackageIndex,
        package: hir::PackageId,
        file: usize,
        rest: &[String],
        local_name: &str,
        witness: scoop_ast::Span,
    ) -> Walked {
        let first = &rest[0];
        let hits = index.hits.get(&(package, first.clone()));
        let mut candidates: Vec<NominalTarget> = Vec::new();
        let mut existed = false;
        if let Some(hits) = hits {
            fn visible<T>(hit: &Hit<T>, file: usize) -> bool {
                !hit.private || hit.declaring_file == file
            }
            for hit in &hits.structs {
                existed = true;
                if visible(hit, file) {
                    candidates.push(NominalTarget::Struct(hit.entity));
                }
            }
            for hit in &hits.enums {
                existed = true;
                if visible(hit, file) {
                    candidates.push(NominalTarget::Enum(hit.entity));
                }
            }
            for hit in &hits.classes {
                existed = true;
                if visible(hit, file) {
                    candidates.push(NominalTarget::Class(hit.entity));
                }
            }
            for hit in &hits.interfaces {
                existed = true;
                if visible(hit, file) {
                    candidates.push(NominalTarget::Interface(hit.entity));
                }
            }
            for hit in &hits.objects {
                existed = true;
                if visible(hit, file) {
                    candidates.push(NominalTarget::Object(hit.entity));
                }
            }
        }
        if candidates.is_empty() {
            return if existed {
                Walked::Existed
            } else {
                Walked::NoMatch
            };
        }
        // Walk every intermediate segment; ambiguity between two visible
        // owners is an error the same way a missing owner is — the
        // selector names one entity.
        let mut resolved: Vec<NominalTarget> = candidates;
        for segment in &rest[1..] {
            let mut next = Vec::new();
            for candidate in &resolved {
                if let Some(target) = self
                    .nested_nominals_by_owner
                    .get(&(candidate.owner(), segment.clone()))
                {
                    next.push(*target);
                }
            }
            if next.is_empty() {
                return if resolved.is_empty() {
                    Walked::NoMatch
                } else {
                    // The owner existed but the nested name does not.
                    Walked::Existed
                };
            }
            resolved = next;
        }
        if resolved.is_empty() {
            return Walked::Existed;
        }
        let source = hir::ImportBindingSource::CurrentCone { witness };
        let targets: Vec<hir::ImportedTargetBinding> = resolved
            .into_iter()
            .map(|target| hir::ImportedTargetBinding {
                target: match target {
                    NominalTarget::Struct(declaration) => {
                        hir::ImportedTarget::Struct { declaration }
                    }
                    NominalTarget::Enum(declaration) => hir::ImportedTarget::Enum { declaration },
                    NominalTarget::Class(declaration) => hir::ImportedTarget::Class { declaration },
                    NominalTarget::Interface(declaration) => {
                        hir::ImportedTarget::Interface { declaration }
                    }
                    NominalTarget::Object(declaration) => {
                        hir::ImportedTarget::Object { declaration }
                    }
                },
                sources: vec![source.clone()],
            })
            .collect();
        Walked::Resolved(hir::ImportedBinding::of(local_name.to_owned(), targets))
    }

    /// The interned package id whose segments equal `segments`, if any;
    /// the empty slice always matches the root package.
    fn package_by_segments(&self, segments: &[String]) -> Option<hir::PackageId> {
        self.package_decls
            .iter()
            .position(|package| package.segments.as_slice() == segments)
            .map(|index| hir::PackageId::from_raw(la_arena::RawIdx::from_u32(index as u32)))
    }
}

enum Walked {
    Resolved(hir::ImportedBinding),
    /// Some declaration existed along the path but none was visible.
    Existed,
    NoMatch,
}
