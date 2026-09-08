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

    /// The kind word of the first same-name type-namespace entry that is
    /// not the re-export target's own origin. The type namespace is
    /// shared by every nominal kind and typealiases, so a same-name
    /// entry of any kind conflicts (DESIGN 2.4); the target's own
    /// origin declaration merges instead.
    fn type_conflict_kind(&self, target: &hir::ImportedTarget) -> Option<&'static str> {
        fn any_other<T: Copy + PartialEq>(hits: &[Hit<T>], origin: Option<T>) -> bool {
            hits.iter().any(|hit| Some(hit.entity) != origin)
        }
        let (
            struct_origin,
            enum_origin,
            class_origin,
            interface_origin,
            object_origin,
            alias_origin,
        ) = match target {
            hir::ImportedTarget::Struct { declaration } => {
                (Some(*declaration), None, None, None, None, None)
            }
            hir::ImportedTarget::Enum { declaration } => {
                (None, Some(*declaration), None, None, None, None)
            }
            hir::ImportedTarget::Class { declaration } => {
                (None, None, Some(*declaration), None, None, None)
            }
            hir::ImportedTarget::Interface { declaration } => {
                (None, None, None, Some(*declaration), None, None)
            }
            hir::ImportedTarget::Object { declaration } => {
                (None, None, None, None, Some(*declaration), None)
            }
            hir::ImportedTarget::TypeAlias { alias } => {
                (None, None, None, None, None, Some(*alias))
            }
            hir::ImportedTarget::Function { .. }
            | hir::ImportedTarget::Property { .. }
            | hir::ImportedTarget::Variant { .. } => (None, None, None, None, None, None),
        };
        if any_other(&self.structs, struct_origin) {
            return Some("a struct");
        }
        if any_other(&self.enums, enum_origin) {
            return Some("an enum");
        }
        if any_other(&self.classes, class_origin) {
            return Some("a class");
        }
        if any_other(&self.interfaces, interface_origin) {
            return Some("an interface");
        }
        if any_other(&self.objects, object_origin) {
            return Some("an object");
        }
        if any_other(&self.type_aliases, alias_origin) {
            return Some("a typealias");
        }
        None
    }
}

/// (package id, top-level name) -> namespace hits.
struct PackageIndex {
    hits: HashMap<(hir::PackageId, String), NamespaceHits>,
}

impl Lowerer {
    /// Resolves every file's imports in deterministic file and
    /// declaration order (exact imports first, then stars); unresolved
    /// or invisible selectors are diagnosed once per import. Star-
    /// imported enum variant surfaces feed the per-file variant
    /// candidate layers. Runs before any signature or body position so
    /// every consumer sees the exact-import layer filled.
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
                        // The local name (alias or source short name)
                        // indexes the targets for the exact-import
                        // lookup layer (spec 12.4.1).
                        for target_binding in &binding.targets {
                            self.file_import_bindings[file]
                                .entry(local_name.clone())
                                .or_default()
                                .push(target_binding.clone());
                        }
                        // The local binding keeps every resolved target;
                        // re-export authorization only gates publication.
                        self.file_imports[file].exact[import_index].binding = Some(binding);
                    }
                    Err(message) => self.import_error(file, span, message),
                }
            }
            for star_index in 0..self.file_imports[file].star.len() {
                let (path, span) = {
                    let star = &self.file_imports[file].star[star_index];
                    (star.path.clone(), star.span)
                };
                match self.resolve_star(&index, file, &path) {
                    Ok(surface) => {
                        if let hir::StarSurface::EnumOwner { declaration } = surface {
                            // The enum's variant short names join this
                            // file's star variant layer, deduplicated by
                            // typed origin across repeated star paths.
                            for variant_index in 0..self.enums[declaration].variants.len() as u32 {
                                let variant = hir::EnumVariantRef::checked(
                                    &self.enums,
                                    declaration,
                                    variant_index,
                                )
                                .expect("iterated variant belongs to its enum");
                                let name = self.enums[declaration].variants[variant_index as usize]
                                    .name
                                    .clone();
                                let refs = self.file_star_variants[file].entry(name).or_default();
                                if !refs.contains(&variant) {
                                    refs.push(variant);
                                }
                            }
                        }
                        self.file_imports[file].star[star_index].binding = Some(surface);
                    }
                    Err(message) => self.import_error(file, span, message),
                }
            }
        }
    }

    /// Publishes re-exports from every resolved `public import` (DESIGN
    /// 2.4) in deterministic file and declaration order. Runs after
    /// signatures exist so destination-conflict checks can compare
    /// parameter signatures.
    pub(super) fn publish_reexports(&mut self) {
        let index = self.build_package_index();
        let mut reexports = Vec::new();
        for file in 0..self.file_imports.len() {
            for import_index in 0..self.file_imports[file].exact.len() {
                let (public, binding, span) = {
                    let import = &self.file_imports[file].exact[import_index];
                    (import.public, import.binding.clone(), import.span)
                };
                if public && let Some(binding) = binding {
                    self.publish_reexport(&index, &mut reexports, file, &binding, span);
                }
            }
            for star_index in 0..self.file_imports[file].star.len() {
                let (public, surface, path, span) = {
                    let star = &self.file_imports[file].star[star_index];
                    (
                        star.public,
                        star.binding.clone(),
                        star.path.clone(),
                        star.span,
                    )
                };
                if public && let Some(surface) = surface {
                    self.expand_public_star(&index, &mut reexports, file, &surface, &path, span);
                }
            }
        }
        self.reexports = reexports;
    }

    /// Reports one import-resolution diagnostic attributed to the
    /// importing file; body lowering is not active at this point, so
    /// `current_file` cannot be relied on.
    fn import_error(&mut self, file: usize, span: scoop_ast::Span, message: String) {
        let mut diagnostic = Diagnostic::at(span, message);
        diagnostic.file = file;
        self.diagnostics.push(diagnostic);
    }

    /// The current file's exact-import targets bound under `name`
    /// (alias or source short name).
    pub(crate) fn exact_import_bindings(&self, name: &str) -> &[hir::ImportedTargetBinding] {
        self.file_import_bindings[self.current_file]
            .get(name)
            .map_or(&[], Vec::as_slice)
    }

    /// The current file's alias- or short-name-bound function targets.
    pub(crate) fn exact_import_functions(&self, name: &str) -> Vec<hir::FunctionId> {
        self.exact_import_bindings(name)
            .iter()
            .filter_map(|binding| match binding.target {
                hir::ImportedTarget::Function { function } => Some(function),
                _ => None,
            })
            .collect()
    }

    /// The current file's alias- or short-name-bound top-level property
    /// targets.
    pub(crate) fn exact_import_properties(&self, name: &str) -> Vec<hir::PropertyId> {
        self.exact_import_bindings(name)
            .iter()
            .filter_map(|binding| match binding.target {
                hir::ImportedTarget::Property { property } => Some(property),
                _ => None,
            })
            .collect()
    }

    /// The current file's alias- or short-name-bound nominal and alias
    /// targets, in binding order.
    pub(crate) fn exact_import_nominals(&self, name: &str) -> Vec<NominalTarget> {
        self.exact_import_bindings(name)
            .iter()
            .filter_map(|binding| match binding.target {
                hir::ImportedTarget::Struct { declaration } => {
                    Some(NominalTarget::Struct(declaration))
                }
                hir::ImportedTarget::Enum { declaration } => Some(NominalTarget::Enum(declaration)),
                hir::ImportedTarget::Class { declaration } => {
                    Some(NominalTarget::Class(declaration))
                }
                hir::ImportedTarget::Interface { declaration } => {
                    Some(NominalTarget::Interface(declaration))
                }
                hir::ImportedTarget::Object { declaration } => {
                    Some(NominalTarget::Object(declaration))
                }
                // Typealias targets are consumed through
                // `source_type_alias_named`, not the nominal maps.
                _ => None,
            })
            .collect()
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
        let resolved = match self.nominal_path_candidates(index, package, file, rest) {
            NominalWalk::Reached(candidates) => candidates,
            NominalWalk::Existed => return Walked::Existed,
            NominalWalk::NoMatch => return Walked::NoMatch,
        };
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

    /// Resolves `rest` (at least one segment) against a package: the
    /// first segment names a top-level nominal, later segments walk
    /// nested nominal owners. Non-nominal members are not reachable
    /// through an owner path; ambiguity between two visible owners is
    /// surfaced by returning both candidates for the caller to report.
    fn nominal_path_candidates(
        &self,
        index: &PackageIndex,
        package: hir::PackageId,
        file: usize,
        rest: &[String],
    ) -> NominalWalk {
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
                NominalWalk::Existed
            } else {
                NominalWalk::NoMatch
            };
        }
        let mut resolved = candidates;
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
                // The owner existed but the nested name does not.
                return NominalWalk::Existed;
            }
            resolved = next;
        }
        NominalWalk::Reached(resolved)
    }

    /// Resolves one star selector (DESIGN 2.2): the longest
    /// declared-package prefix wins, remaining segments walk nested
    /// nominal owners, and the final entity must be the declared package
    /// itself or one enum (its variant surface). Stars naming other
    /// nominal owners are undefined in v1; every failure is diagnosed
    /// once per import.
    fn resolve_star(
        &self,
        index: &PackageIndex,
        file: usize,
        path: &[String],
    ) -> Result<hir::StarSurface, String> {
        let path_text = format!("{}.*", path.join("."));
        let mut existed_somewhere = false;
        let mut reached_error: Option<String> = None;
        // k = number of package-prefix segments; the full path may
        // itself be the package, so k ranges over 0..=path.len().
        for k in (0..=path.len()).rev() {
            if k == path.len() {
                if let Some(package) = self.package_by_segments(path) {
                    return Ok(hir::StarSurface::Package { package });
                }
                continue;
            }
            let package = match self.package_by_segments(&path[..k]) {
                Some(package) => package,
                None => continue,
            };
            let rest = &path[k..];
            match self.nominal_path_candidates(index, package, file, rest) {
                NominalWalk::Reached(candidates) => {
                    let enums: Vec<hir::EnumId> = candidates
                        .iter()
                        .filter_map(|target| match target {
                            NominalTarget::Enum(id) => Some(*id),
                            _ => None,
                        })
                        .collect();
                    match enums.as_slice() {
                        [enumeration] => {
                            return Ok(hir::StarSurface::EnumOwner {
                                declaration: *enumeration,
                            });
                        }
                        [] => reached_error.get_or_insert(format!(
                            "star import `{path_text}` must name a package or an enum"
                        )),
                        more => reached_error.get_or_insert(format!(
                            "star import `{path_text}` is ambiguous between {} enums",
                            more.len()
                        )),
                    };
                }
                NominalWalk::Existed => existed_somewhere = true,
                NominalWalk::NoMatch => {}
            }
        }
        if let Some(message) = reached_error {
            Err(message)
        } else if existed_somewhere {
            Err(format!(
                "star import `{path_text}` names declarations that are not visible in this file"
            ))
        } else {
            Err(format!("unresolved star import `{path_text}`"))
        }
    }

    /// Expands one `public import ... .*` into per-name re-export
    /// bindings (DESIGN 2.4): a package surface publishes every public
    /// member short name (overload sets share one binding); an enum
    /// surface publishes one `Variant` target per variant. Internal or
    /// private members silently stay out of the snapshot; a surface
    /// with public members but no publishable target is diagnosed once.
    fn expand_public_star(
        &mut self,
        index: &PackageIndex,
        reexports: &mut Vec<hir::ReExport>,
        file: usize,
        surface: &hir::StarSurface,
        path: &[String],
        span: scoop_ast::Span,
    ) {
        let destination = self.file_packages[file];
        let path_text = format!("{}.*", path.join("."));
        let empty_surface = |this: &mut Self| {
            this.import_error(
                file,
                span,
                format!(
                    "public import `{path_text}` publishes no target: none is provided by a direct dependency"
                ),
            );
        };
        match surface {
            hir::StarSurface::Package { package } => {
                let mut groups: Vec<(&String, &NamespaceHits)> = index
                    .hits
                    .iter()
                    .filter(|((hit_package, _), _)| *hit_package == *package)
                    .map(|((_, name), hits)| (name, hits))
                    .collect();
                groups.sort_by(|a, b| a.0.cmp(b.0));
                let mut published_any = false;
                let mut surface_has_public = false;
                for (name, hits) in groups {
                    let targets = self.single_segment_targets(hits, file, span);
                    if targets.is_empty() {
                        continue;
                    }
                    if targets
                        .iter()
                        .any(|target| self.imported_target_is_public(&target.target))
                    {
                        surface_has_public = true;
                    }
                    let mut published = Vec::new();
                    for target_binding in targets {
                        if !self.imported_target_is_public(&target_binding.target) {
                            continue;
                        }
                        let declaring_file = self.imported_target_file(&target_binding.target);
                        if !self.intrinsic_sources[declaring_file].core {
                            continue;
                        }
                        if let Some(message) = self.reexport_destination_conflict(
                            index,
                            reexports,
                            destination,
                            name,
                            &target_binding,
                        ) {
                            self.import_error(file, span, message);
                            continue;
                        }
                        published.push(target_binding);
                    }
                    if !published.is_empty() {
                        published_any = true;
                        reexports.push(hir::ReExport {
                            package: destination,
                            name: name.clone(),
                            span,
                            binding: hir::ImportedBinding::of(name.clone(), published),
                        });
                    }
                }
                if !published_any && surface_has_public {
                    empty_surface(self);
                }
            }
            hir::StarSurface::EnumOwner { declaration } => {
                let enum_file = self.enums[*declaration].origin.file as usize;
                if !self.intrinsic_sources[enum_file].core {
                    empty_surface(self);
                    return;
                }
                let variant_count = self.enums[*declaration].variants.len();
                for variant_index in 0..variant_count as u32 {
                    let name = self.enums[*declaration].variants[variant_index as usize]
                        .name
                        .clone();
                    let variant =
                        hir::EnumVariantRef::checked(&self.enums, *declaration, variant_index)
                            .expect("iterated variant belongs to its enum");
                    let target_binding = hir::ImportedTargetBinding {
                        target: hir::ImportedTarget::Variant { variant },
                        sources: vec![hir::ImportBindingSource::CurrentCone { witness: span }],
                    };
                    if let Some(message) = self.reexport_destination_conflict(
                        index,
                        reexports,
                        destination,
                        &name,
                        &target_binding,
                    ) {
                        self.import_error(file, span, message);
                        continue;
                    }
                    reexports.push(hir::ReExport {
                        package: destination,
                        name: name.clone(),
                        span,
                        binding: hir::ImportedBinding::of(name, vec![target_binding]),
                    });
                }
            }
        }
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

/// The owner-walk outcome for a nominal path, shared by exact imports
/// and star-surface resolution.
enum NominalWalk {
    /// The candidate set that survived the walk; same-name hits in
    /// distinct namespaces all survive for the caller to classify.
    Reached(Vec<NominalTarget>),
    /// Some declaration existed along the path but none was visible.
    Existed,
    NoMatch,
}

impl Lowerer {
    /// Validates one `public import` binding (DESIGN 2.4) and records the
    /// authorized re-export. In the transitional single-Cone model the
    /// implicitly imported core unit is the only direct edge: a target
    /// declared by a core-unit file is publishable, while a target from
    /// the current Cone's own user code must reach the public surface
    /// through its package's ordinary export rules and cannot be copied
    /// into a re-export. Destination names colliding with a
    /// non-overloadable same-package declaration or a different-origin
    /// re-export are diagnosed before the surface is published.
    fn publish_reexport(
        &mut self,
        index: &PackageIndex,
        reexports: &mut Vec<hir::ReExport>,
        file: usize,
        binding: &hir::ImportedBinding,
        span: scoop_ast::Span,
    ) {
        let package = self.file_packages[file];
        let mut published = Vec::new();
        for target_binding in &binding.targets {
            let name = self.imported_target_name(&target_binding.target);
            let declaring_file = self.imported_target_file(&target_binding.target);
            if !self.intrinsic_sources[declaring_file].core {
                self.import_error(
                    file,
                    span,
                    format!("public import target `{name}` is not provided by a direct dependency"),
                );
                continue;
            }
            if !self.imported_target_is_public(&target_binding.target) {
                self.import_error(
                    file,
                    span,
                    format!(
                        "public import target `{name}` is not public and cannot be re-exported"
                    ),
                );
                continue;
            }
            if let Some(message) = self.reexport_destination_conflict(
                index,
                reexports,
                package,
                &binding.local_name,
                target_binding,
            ) {
                self.import_error(file, span, message);
                continue;
            }
            published.push(target_binding.clone());
        }
        if !published.is_empty() {
            reexports.push(hir::ReExport {
                package,
                name: binding.local_name.clone(),
                span,
                binding: hir::ImportedBinding::of(binding.local_name.clone(), published),
            });
        }
    }

    /// The source-level short name of one import target.
    fn imported_target_name(&self, target: &hir::ImportedTarget) -> String {
        match target {
            hir::ImportedTarget::Function { function } => self.functions[*function].name.clone(),
            hir::ImportedTarget::Struct { declaration } => self.structs[*declaration].name.clone(),
            hir::ImportedTarget::Enum { declaration } => self.enums[*declaration].name.clone(),
            hir::ImportedTarget::Class { declaration } => self.classes[*declaration].name.clone(),
            hir::ImportedTarget::Interface { declaration } => {
                self.interfaces[*declaration].name.clone()
            }
            hir::ImportedTarget::Object { declaration } => self.objects[*declaration].name.clone(),
            hir::ImportedTarget::TypeAlias { alias } => self.type_aliases[*alias].name.clone(),
            hir::ImportedTarget::Property { property } => self.properties[*property].name.clone(),
            hir::ImportedTarget::Variant { variant } => self.enums[variant.enumeration()].variants
                [variant.local_index() as usize]
                .name
                .clone(),
        }
    }

    /// The declaring file of one import target, from the same origins the
    /// package index was built from.
    fn imported_target_file(&self, target: &hir::ImportedTarget) -> usize {
        match target {
            hir::ImportedTarget::Function { function } => {
                self.functions[*function].origin.file as usize
            }
            hir::ImportedTarget::Struct { declaration } => {
                self.structs[*declaration].origin.file as usize
            }
            hir::ImportedTarget::Enum { declaration } => {
                self.enums[*declaration].origin.file as usize
            }
            hir::ImportedTarget::Class { declaration } => {
                self.classes[*declaration].origin.file as usize
            }
            hir::ImportedTarget::Interface { declaration } => {
                self.interfaces[*declaration].origin.file as usize
            }
            hir::ImportedTarget::Object { declaration } => {
                // Objects ride on their backing class's origin.
                self.classes[self.objects[*declaration].backing_class]
                    .origin
                    .file as usize
            }
            hir::ImportedTarget::TypeAlias { alias } => {
                self.type_aliases[*alias].origin.file as usize
            }
            hir::ImportedTarget::Property { property } => self.property_files[property],
            hir::ImportedTarget::Variant { variant } => {
                self.enums[variant.enumeration()].origin.file as usize
            }
        }
    }

    /// Whether one import target is declared `public`; a re-export cannot
    /// raise a target's own visibility (DESIGN 2.5).
    fn imported_target_is_public(&self, target: &hir::ImportedTarget) -> bool {
        let is_public =
            |declared: DeclaredVisibility| matches!(declared, DeclaredVisibility::Public);
        match target {
            hir::ImportedTarget::Function { function } => {
                is_public(self.functions[*function].access.declared)
            }
            hir::ImportedTarget::Struct { declaration } => {
                is_public(self.structs[*declaration].access.declared)
            }
            hir::ImportedTarget::Enum { declaration } => {
                is_public(self.enums[*declaration].access.declared)
            }
            hir::ImportedTarget::Class { declaration } => {
                is_public(self.classes[*declaration].access.declared)
            }
            hir::ImportedTarget::Interface { declaration } => {
                is_public(self.interfaces[*declaration].access.declared)
            }
            hir::ImportedTarget::Object { declaration } => {
                is_public(self.objects[*declaration].access.declared)
            }
            hir::ImportedTarget::TypeAlias { alias } => {
                is_public(self.type_aliases[*alias].access.declared)
            }
            hir::ImportedTarget::Property { property } => {
                is_public(self.properties[*property].access.declared)
            }
            // Variants are fixed public (spec 4.2: no visibility
            // modifiers on variants).
            hir::ImportedTarget::Variant { .. } => true,
        }
    }

    /// The destination conflict for one target (DESIGN 2.4): a same-name
    /// entry of the same namespace that cannot legally overload. The
    /// target's own origin merges instead of conflicting, whether it is
    /// a local declaration in the destination package or an earlier
    /// re-export of the same entity.
    fn reexport_destination_conflict(
        &self,
        index: &PackageIndex,
        reexports: &[hir::ReExport],
        package: hir::PackageId,
        destination: &str,
        target: &hir::ImportedTargetBinding,
    ) -> Option<String> {
        let package_display = self.package_decls[u32::from(package.into_raw()) as usize].display();
        let conflict = |kind: &str| {
            format!(
                "public import re-export `{destination}` conflicts with {kind} in package `{package_display}`"
            )
        };
        // Earlier re-exports into the same destination.
        for reexport in reexports {
            if reexport.package != package || reexport.name != destination {
                continue;
            }
            for prior in &reexport.binding.targets {
                if prior.target == target.target {
                    // Same origin: the binding merges.
                    continue;
                }
                match (&prior.target, &target.target) {
                    (
                        hir::ImportedTarget::Struct { .. }
                        | hir::ImportedTarget::Enum { .. }
                        | hir::ImportedTarget::Class { .. }
                        | hir::ImportedTarget::Interface { .. }
                        | hir::ImportedTarget::Object { .. }
                        | hir::ImportedTarget::TypeAlias { .. },
                        hir::ImportedTarget::Struct { .. }
                        | hir::ImportedTarget::Enum { .. }
                        | hir::ImportedTarget::Class { .. }
                        | hir::ImportedTarget::Interface { .. }
                        | hir::ImportedTarget::Object { .. }
                        | hir::ImportedTarget::TypeAlias { .. },
                    ) => {
                        return Some(conflict(imported_target_kind(&prior.target)));
                    }
                    (
                        hir::ImportedTarget::Function { function: previous },
                        hir::ImportedTarget::Function { function },
                    ) => {
                        if self.same_parameter_signature(*previous, *function) {
                            return Some(conflict("a function with the same signature"));
                        }
                    }
                    (
                        hir::ImportedTarget::Property { .. },
                        hir::ImportedTarget::Property { .. },
                    ) => {
                        return Some(conflict("a property"));
                    }
                    // Variant names are constructor-like and not
                    // overloadable: any same-name variant, function,
                    // property or type at the destination conflicts.
                    (hir::ImportedTarget::Variant { .. }, hir::ImportedTarget::Variant { .. }) => {
                        return Some(conflict("an enum variant"));
                    }
                    (
                        hir::ImportedTarget::Variant { .. },
                        hir::ImportedTarget::Function { .. }
                        | hir::ImportedTarget::Property { .. }
                        | hir::ImportedTarget::Struct { .. }
                        | hir::ImportedTarget::Enum { .. }
                        | hir::ImportedTarget::Class { .. }
                        | hir::ImportedTarget::Interface { .. }
                        | hir::ImportedTarget::Object { .. }
                        | hir::ImportedTarget::TypeAlias { .. },
                    ) => {
                        return Some(conflict("an enum variant"));
                    }
                    (
                        hir::ImportedTarget::Function { .. }
                        | hir::ImportedTarget::Property { .. }
                        | hir::ImportedTarget::Struct { .. }
                        | hir::ImportedTarget::Enum { .. }
                        | hir::ImportedTarget::Class { .. }
                        | hir::ImportedTarget::Interface { .. }
                        | hir::ImportedTarget::Object { .. }
                        | hir::ImportedTarget::TypeAlias { .. },
                        hir::ImportedTarget::Variant { .. },
                    ) => {
                        return Some(conflict(imported_target_kind(&prior.target)));
                    }
                    _ => {}
                }
            }
        }
        // Local declarations in the destination package; the index keys
        // hits by the declaring file's package, so this lookup is already
        // package-scoped.
        let hits = index.hits.get(&(package, destination.to_owned()))?;
        match &target.target {
            hir::ImportedTarget::Function { function } => {
                for hit in &hits.functions {
                    if hit.entity == *function {
                        // Same origin: the binding merges.
                        continue;
                    }
                    if self.same_parameter_signature(*function, hit.entity) {
                        return Some(conflict("a function with the same signature"));
                    }
                }
                None
            }
            hir::ImportedTarget::Property { property } => hits
                .properties
                .iter()
                .any(|hit| hit.entity != *property)
                .then(|| conflict("a property")),
            hir::ImportedTarget::Variant { .. } => {
                if !hits.functions.is_empty() {
                    return Some(conflict("a function"));
                }
                if !hits.properties.is_empty() {
                    return Some(conflict("a property"));
                }
                hits.type_conflict_kind(&target.target).map(conflict)
            }
            _ => hits.type_conflict_kind(&target.target).map(conflict),
        }
    }
}

/// The diagnostic kind word for one import target's namespace role.
fn imported_target_kind(target: &hir::ImportedTarget) -> &'static str {
    match target {
        hir::ImportedTarget::Struct { .. } => "a struct",
        hir::ImportedTarget::Enum { .. } => "an enum",
        hir::ImportedTarget::Class { .. } => "a class",
        hir::ImportedTarget::Interface { .. } => "an interface",
        hir::ImportedTarget::Object { .. } => "an object",
        hir::ImportedTarget::TypeAlias { .. } => "a typealias",
        hir::ImportedTarget::Function { .. } => "a function",
        hir::ImportedTarget::Property { .. } => "a property",
        hir::ImportedTarget::Variant { .. } => "an enum variant",
    }
}

/// The DESIGN 2.3 layer order for unqualified top-level lookops between
/// the this-member layer and the core-prelude variant layer:
/// exact imports shadow the current package, which shadows star imports,
/// which shadow the implicitly imported core surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LookupLayer {
    ExactImport,
    CurrentPackage,
    StarImport,
    CorePrelude,
}

pub(crate) const LOOKUP_LAYERS: [LookupLayer; 4] = [
    LookupLayer::ExactImport,
    LookupLayer::CurrentPackage,
    LookupLayer::StarImport,
    LookupLayer::CorePrelude,
];

impl Lowerer {
    /// Classifies one declaration (by its declaring file and whether the
    /// current file's exact imports bind this specific entity) into the
    /// first layer that admits it; `None` means invisible at unqualified
    /// level.
    pub(crate) fn lookup_layer(
        &self,
        declaring_file: usize,
        entity_imported: bool,
    ) -> Option<LookupLayer> {
        if entity_imported {
            return Some(LookupLayer::ExactImport);
        }
        if self.file_packages[declaring_file] == self.file_packages[self.current_file] {
            return Some(LookupLayer::CurrentPackage);
        }
        let declaring_package = self.file_packages[declaring_file];
        if self.file_star_imports_package(self.current_file, declaring_package) {
            return Some(LookupLayer::StarImport);
        }
        if self.intrinsic_sources[declaring_file].core {
            return Some(LookupLayer::CorePrelude);
        }
        None
    }

    /// Whether one of the file's star imports resolved to exactly this
    /// package surface.
    fn file_star_imports_package(&self, file: usize, package: hir::PackageId) -> bool {
        self.file_imports[file].star.iter().any(|star| {
            matches!(
                &star.binding,
                Some(hir::StarSurface::Package { package: imported }) if *imported == package
            )
        })
    }

    /// Whether the current file's resolved exact-import bindings contain
    /// this function.
    fn function_exact_imported(&self, function: hir::FunctionId) -> bool {
        self.file_imports[self.current_file]
            .exact
            .iter()
            .filter_map(|import| import.binding.as_ref())
            .any(|binding| {
                binding.targets.iter().any(|target| {
                    matches!(
                        target.target,
                        hir::ImportedTarget::Function { function: imported } if imported == function
                    )
                })
            })
    }

    /// The lookup layer of one top-level function from the current file.
    pub(crate) fn function_lookup_layer(&self, function: hir::FunctionId) -> Option<LookupLayer> {
        self.lookup_layer(
            self.function_files[&function],
            self.function_exact_imported(function),
        )
    }

    /// The lookup layer of one top-level property from the current file.
    pub(crate) fn property_lookup_layer(&self, property: hir::PropertyId) -> Option<LookupLayer> {
        let entity_imported = self.file_imports[self.current_file]
            .exact
            .iter()
            .filter_map(|import| import.binding.as_ref())
            .any(|binding| {
                binding.targets.iter().any(|target| {
                    matches!(
                        target.target,
                        hir::ImportedTarget::Property { property: imported } if imported == property
                    )
                })
            });
        self.lookup_layer(self.property_files[&property], entity_imported)
    }
}

impl Lowerer {
    /// The lookup layer of one source typealias.
    pub(crate) fn source_alias_lookup_layer(
        &self,
        alias: crate::aliases::SourceTypeAliasId,
    ) -> Option<LookupLayer> {
        let entity_imported = self.file_imports[self.current_file]
            .exact
            .iter()
            .filter_map(|import| import.binding.as_ref())
            .any(|binding| {
                binding.targets.iter().any(|target| {
                    matches!(
                        target.target,
                        hir::ImportedTarget::TypeAlias { alias: imported } if Some(&imported)
                            == self.export_alias_ids.get(&alias)
                    )
                })
            });
        let declaring_file = self.source_type_aliases[alias].file();
        self.lookup_layer(declaring_file, entity_imported)
    }
}

impl Lowerer {
    /// All top-level nominal candidates for `name` admitted at one
    /// lookup layer, in declaration order. Non-overloadable: a layer
    /// with more than one candidate is an ambiguity the caller reports.
    /// The lookup layer of one nominal declaration, honouring exact
    /// imports that bind this specific entity.
    pub(crate) fn nominal_lookup_layer(&self, target: NominalTarget) -> Option<LookupLayer> {
        let entity_imported = self.file_imports[self.current_file]
            .exact
            .iter()
            .filter_map(|import| import.binding.as_ref())
            .any(|binding| {
                binding.targets.iter().any(|candidate| {
                    let imported = &candidate.target;
                    match (imported, target) {
                        (
                            hir::ImportedTarget::Struct { declaration },
                            NominalTarget::Struct(actual),
                        ) => *declaration == actual,
                        (
                            hir::ImportedTarget::Enum { declaration },
                            NominalTarget::Enum(actual),
                        ) => *declaration == actual,
                        (
                            hir::ImportedTarget::Class { declaration },
                            NominalTarget::Class(actual),
                        ) => *declaration == actual,
                        (
                            hir::ImportedTarget::Interface { declaration },
                            NominalTarget::Interface(actual),
                        ) => *declaration == actual,
                        (
                            hir::ImportedTarget::Object { declaration },
                            NominalTarget::Object(actual),
                        ) => *declaration == actual,
                        _ => false,
                    }
                })
            });
        // Every declared nominal registers its declaring file.
        let declaring_file = match target {
            NominalTarget::Struct(id) => self.struct_files[&id],
            NominalTarget::Enum(id) => self.enum_files[&id],
            NominalTarget::Class(id) => self.class_files[&id],
            NominalTarget::Interface(id) => self.interface_files[&id],
            NominalTarget::Object(id) => self.object_files[&id],
        };
        self.lookup_layer(declaring_file, entity_imported)
    }

    pub(crate) fn nominal_candidates_in_layer(
        &self,
        name: &str,
        layer: LookupLayer,
    ) -> Vec<NominalTarget> {
        let mut candidates = Vec::new();
        candidates.extend(
            self.structs_by_name
                .get(name)
                .into_iter()
                .flatten()
                .map(|(id, _)| NominalTarget::Struct(*id)),
        );
        candidates.extend(
            self.enums_by_name
                .get(name)
                .into_iter()
                .flatten()
                .map(|id| NominalTarget::Enum(*id)),
        );
        candidates.extend(
            self.classes_by_name
                .get(name)
                .into_iter()
                .flatten()
                .map(|(id, _)| NominalTarget::Class(*id)),
        );
        candidates.extend(
            self.interfaces_by_name
                .get(name)
                .into_iter()
                .flatten()
                .map(|(id, _)| NominalTarget::Interface(*id)),
        );
        candidates.extend(
            self.objects_by_name
                .get(name)
                .into_iter()
                .flatten()
                .map(|id| NominalTarget::Object(*id)),
        );
        if layer == LookupLayer::ExactImport {
            // Alias-bound imports: the exact-import layer admits targets
            // through the local name even when the source name differs.
            for target in self.exact_import_nominals(name) {
                if !candidates.contains(&target) {
                    candidates.push(target);
                }
            }
        }
        candidates.retain(|target| self.nominal_lookup_layer(*target) == Some(layer));
        candidates
    }

    /// The first layer (in DESIGN order) admitting at least one nominal
    /// candidate for `name`, with its candidates. Unqualified type
    /// lookup is non-overloadable: multiple candidates in one layer are
    /// reported by the caller as ambiguity.
    pub(crate) fn first_nominal_layer(
        &self,
        name: &str,
    ) -> Option<(LookupLayer, Vec<NominalTarget>)> {
        for layer in LOOKUP_LAYERS {
            let candidates = self.nominal_candidates_in_layer(name, layer);
            if !candidates.is_empty() {
                return Some((layer, candidates));
            }
        }
        None
    }

    /// Core-unit well-known nominal lookup for core-contract
    /// validation: the unique candidate declared inside the implicitly
    /// imported core unit.
    pub(crate) fn core_unit_nominal(&self, name: &str) -> Option<NominalTarget> {
        // Core-contract validation asks for the declaration inside the
        // implicitly imported core unit itself; the general layer
        // classifier cannot express this (root-package core files read
        // as the current package from root-package user files).
        let is_core_file = |file: usize| self.intrinsic_sources[file].core;
        if let Some(entries) = self.structs_by_name.get(name) {
            for (id, _) in entries {
                if is_core_file(self.struct_files[id]) {
                    return Some(NominalTarget::Struct(*id));
                }
            }
        }
        if let Some(entries) = self.enums_by_name.get(name) {
            for id in entries {
                if is_core_file(self.enum_files[id]) {
                    return Some(NominalTarget::Enum(*id));
                }
            }
        }
        if let Some(entries) = self.classes_by_name.get(name) {
            for (id, _) in entries {
                if is_core_file(self.class_files[id]) {
                    return Some(NominalTarget::Class(*id));
                }
            }
        }
        if let Some(entries) = self.interfaces_by_name.get(name) {
            for (id, _) in entries {
                if is_core_file(self.interface_files[id]) {
                    return Some(NominalTarget::Interface(*id));
                }
            }
        }
        if let Some(entries) = self.objects_by_name.get(name) {
            for id in entries {
                if is_core_file(self.object_files[id]) {
                    return Some(NominalTarget::Object(*id));
                }
            }
        }
        None
    }
}
