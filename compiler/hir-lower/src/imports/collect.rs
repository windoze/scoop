use super::*;
use crate::{
    Lowerer, NominalTarget, Owner, SourceKind,
    namespace::{TopLevelLookupLayer, TopLevelTypeTarget},
};

impl Lowerer {
    fn import_source(&self, file: usize) -> scoop_identity::SourceIdentity {
        debug_assert_eq!(self.intrinsic_sources[file].kind, SourceKind::CurrentUnit);
        self.visibility_file(file)
    }

    fn import_package(&self, file: usize) -> PackageId {
        match self.top_level_namespaces.source_namespace(file) {
            TopLevelLookupLayer::CurrentPackage(package) => package,
            TopLevelLookupLayer::CorePrelude => unreachable!("core has no import package"),
        }
    }

    fn static_import_namespace(&self, owner: Owner) -> StaticNamespace {
        match owner {
            Owner::Class(id) => StaticNamespace::Class(id),
            Owner::Interface(id) => StaticNamespace::Interface(id),
            Owner::Struct(id) => StaticNamespace::Struct(id),
            Owner::Enum(id) => StaticNamespace::Enum(id),
            Owner::Object(id) => match self.objects[id].kind {
                hir::ObjectKind::Standalone => StaticNamespace::Object(id),
                hir::ObjectKind::Companion(relation) => StaticNamespace::Companion(relation),
            },
        }
    }

    fn nominal_import_parts(
        &self,
        target: NominalTarget,
    ) -> (
        CurrentUnitTarget,
        usize,
        ast::Span,
        hir::EffectiveLookupDomain,
    ) {
        match target {
            NominalTarget::Class(id) => (
                CurrentUnitTarget::Class(id),
                self.class_files[&id],
                self.classes[id].span,
                self.classes[id].access.lookup.clone(),
            ),
            NominalTarget::Interface(id) => (
                CurrentUnitTarget::Interface(id),
                self.interface_files[&id],
                self.interfaces[id].span,
                self.interfaces[id].access.lookup.clone(),
            ),
            NominalTarget::Struct(id) => (
                CurrentUnitTarget::Struct(id),
                self.struct_files[&id],
                self.structs[id].span,
                self.structs[id].access.lookup.clone(),
            ),
            NominalTarget::Enum(id) => (
                CurrentUnitTarget::Enum(id),
                self.enum_files[&id],
                self.enums[id].span,
                self.enums[id].access.lookup.clone(),
            ),
            NominalTarget::Object(id) => (
                CurrentUnitTarget::Object(id),
                self.object_files[&id],
                self.objects[id].span,
                self.objects[id].access.lookup.clone(),
            ),
        }
    }

    fn collect_import_nominal(
        &self,
        surface: &mut CurrentUnitImports,
        namespace: ResolvedNamespace,
        name: String,
        nominal: NominalTarget,
    ) {
        let (target, file, span, access) = self.nominal_import_parts(nominal);
        if self.source_is_core(file) {
            return;
        }
        // Named companions may have more than one static spelling. Each edge
        // points at the same declaration-side binding and witness.
        let existing = surface
            .bindings
            .iter()
            .position(|binding| binding.target == target);
        let binding = if let Some(index) = existing {
            let id = CurrentUnitBindingId(index);
            surface
                .namespaces
                .entry(namespace)
                .or_default()
                .entry(name)
                .or_default()
                .push(id);
            id
        } else {
            surface.insert(
                namespace,
                CurrentUnitBinding {
                    target,
                    source: self.import_source(file),
                    file,
                    span,
                    access,
                    name,
                },
            )
        };
        let static_namespace = self.static_import_namespace(nominal.owner());
        surface.static_targets.insert(binding, static_namespace);
        surface
            .namespaces
            .entry(ResolvedNamespace::Static(static_namespace))
            .or_default();
    }

    fn collect_import_property(
        &self,
        surface: &mut CurrentUnitImports,
        namespace: ResolvedNamespace,
        declaration: &ast::PropertyDecl,
        file: usize,
        owner: SourcePropertyOwner,
    ) -> SourcePropertyId {
        let declared = Self::normalized_visibility(declaration.visibility);
        let domain = if declared == hir::DeclaredVisibility::Protected {
            // This shape is rejected by the existing declaration pass; no
            // provisional import may grant access while diagnostics collect.
            hir::AccessDomain::empty()
        } else {
            match owner {
                SourcePropertyOwner::TopLevel => self.top_level_domain(declared, file),
                SourcePropertyOwner::Object(object) => self
                    .member_declared_domain(declared, Owner::Object(object), file)
                    .intersect(self.owner_lookup_domain(Owner::Object(object))),
            }
        };
        let id = SourcePropertyId(surface.source_property_count);
        surface.source_property_count += 1;
        if declaration.receiver_ty.is_some() {
            surface.source_extension_properties.insert(id);
        }
        surface.insert(
            namespace,
            CurrentUnitBinding {
                target: CurrentUnitTarget::SourceProperty(id),
                source: self.import_source(file),
                file,
                span: declaration.name.span,
                access: hir::EffectiveLookupDomain(domain),
                name: declaration.name.text.clone(),
            },
        );
        id
    }

    pub(crate) fn collect_and_resolve_imports(
        &mut self,
        files: &[ast::SourceFile],
        declarations: ImportDeclarationInputs<'_>,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
    ) {
        let ImportDeclarationInputs {
            functions,
            methods,
            properties,
            enumerations,
            objects,
        } = declarations;
        let mut surface = CurrentUnitImports::default();
        if let Some(world) = world
            && let Some(core) = world.direct_provider(scoop_identity::ConeIdentity::CORE)
            && let Some(package) = world.direct_package(&scoop_identity::PackagePath::root())
        {
            for group in package.snapshot_filtered(|provider| provider == core.id()) {
                let targets = group
                    .targets()
                    .iter()
                    .filter(|binding| {
                        matches!(
                            binding.target(),
                            hir::ImportedTarget::Function(_)
                                | hir::ImportedTarget::GenericFunction(_)
                                | hir::ImportedTarget::Property(_)
                                | hir::ImportedTarget::ExtensionProperty(_)
                        )
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if !targets.is_empty() {
                    surface
                        .prelude_dependencies
                        .insert(group.name().as_str().to_owned(), targets);
                }
            }
        }
        for (package, name, target, file) in self.top_level_namespaces.current_type_bindings() {
            let namespace = ResolvedNamespace::Package(package);
            match target {
                TopLevelTypeTarget::Nominal(target) => {
                    self.collect_import_nominal(&mut surface, namespace, name, target)
                }
                TopLevelTypeTarget::Alias(alias) => {
                    let declaration = &self.source_type_aliases[alias];
                    surface.insert(
                        namespace,
                        CurrentUnitBinding {
                            target: CurrentUnitTarget::TypeAlias(alias),
                            source: self.import_source(file),
                            file,
                            span: declaration.origin.span,
                            access: declaration.access.lookup.clone(),
                            name,
                        },
                    );
                }
            }
        }
        for (&(owner, ref name), &target) in &self.nested_nominals_by_owner {
            self.collect_import_nominal(
                &mut surface,
                ResolvedNamespace::Static(self.static_import_namespace(owner)),
                name.clone(),
                target,
            );
        }
        for &(function, declaration, file) in functions {
            if self.source_is_core(file) {
                continue;
            }
            surface.insert(
                ResolvedNamespace::Package(self.import_package(file)),
                CurrentUnitBinding {
                    target: CurrentUnitTarget::Function(function),
                    source: self.import_source(file),
                    file,
                    span: declaration.name.span,
                    access: self.functions[function].access.lookup.clone(),
                    name: declaration.name.text.clone(),
                },
            );
        }
        for &(function, declaration, file, owner) in methods {
            if self.source_is_core(file) || !matches!(owner, Owner::Object(_)) {
                continue;
            }
            surface.insert(
                ResolvedNamespace::Static(self.static_import_namespace(owner)),
                CurrentUnitBinding {
                    target: CurrentUnitTarget::Function(function),
                    source: self.import_source(file),
                    file,
                    span: declaration.name.span,
                    access: self.functions[function].access.lookup.clone(),
                    name: declaration.name.text.clone(),
                },
            );
        }
        for &(declaration, file) in properties {
            if self.source_is_core(file) {
                surface
                    .global_property_sources
                    .push(PropertyImportSource::OutsideCurrentUnitSurface);
                continue;
            }
            let id = self.collect_import_property(
                &mut surface,
                ResolvedNamespace::Package(self.import_package(file)),
                declaration,
                file,
                SourcePropertyOwner::TopLevel,
            );
            surface
                .global_property_sources
                .push(PropertyImportSource::CurrentUnit(id));
        }
        for &(object, source, file) in objects {
            if self.source_is_core(file) {
                continue;
            }
            for (member_index, member) in source.members().iter().enumerate() {
                if let ast::ClassMember::StoredProperty(property) = member {
                    let id = self.collect_import_property(
                        &mut surface,
                        ResolvedNamespace::Static(
                            self.static_import_namespace(Owner::Object(object)),
                        ),
                        property,
                        file,
                        SourcePropertyOwner::Object(object),
                    );
                    surface
                        .object_property_sources
                        .insert((object, member_index), id);
                }
            }
        }
        for &(enumeration, declaration, file) in enumerations {
            if self.source_is_core(file) {
                continue;
            }
            for (index, variant) in declaration.variants.iter().enumerate() {
                let id = SourceVariantId(surface.source_variants.len());
                surface.source_variants.push(SourceVariant {
                    enumeration,
                    index: u32::try_from(index).expect("variant index exceeds u32"),
                });
                assert!(
                    surface
                        .enum_variant_sources
                        .insert(
                            (
                                enumeration,
                                u32::try_from(index).expect("variant index exceeds u32"),
                            ),
                            id,
                        )
                        .is_none(),
                    "a source enum position identifies exactly one variant"
                );
                surface.insert(
                    ResolvedNamespace::Static(StaticNamespace::Enum(enumeration)),
                    CurrentUnitBinding {
                        target: CurrentUnitTarget::SourceVariant(id),
                        source: self.import_source(file),
                        file,
                        span: variant.name.span,
                        access: self.enums[enumeration].access.lookup.clone(),
                        name: variant.name.text.clone(),
                    },
                );
            }
        }
        // Companion forwarding is a static namespace edge, never a copied
        // function/property definition or a hidden source re-export. Read
        // every edge from the direct-member snapshot so a companion nested
        // inside another companion cannot be forwarded transitively based on
        // HashMap iteration order.
        let direct_namespaces = surface.namespaces.clone();
        for (&host, &relation) in &self.companion_by_host {
            let object = self.companion_relations[relation].object;
            if self.source_is_core(self.object_files[&object]) {
                continue;
            }
            let from = ResolvedNamespace::Static(StaticNamespace::Companion(relation));
            let to = ResolvedNamespace::Static(self.static_import_namespace(host));
            let members = direct_namespaces.get(&from).cloned().unwrap_or_default();
            for (name, bindings) in members {
                surface
                    .namespaces
                    .entry(to)
                    .or_default()
                    .entry(name)
                    .or_insert(bindings);
            }
        }
        self.current_owner = None;
        for (file, syntax) in files.iter().enumerate() {
            self.current_file = file;
            let imports = if self.source_kind(file) == SourceKind::CurrentUnit {
                surface.resolve_file_with_world(self, syntax, world)
            } else {
                FrozenFileImports::default()
            };
            surface.files.push(imports);
        }
        if let Err(error) = surface.freeze_reexports(self) {
            let mut diagnostic = ast::Diagnostic::at(error.span(), error.to_string());
            diagnostic.file = error.file();
            self.diagnostics.push(diagnostic);
        }
        self.imports = surface;
    }

    /// Resolve source-only property/variant ids before body lookup consumes
    /// the frozen import membership. A declaration which already failed is
    /// removed from every semantic scope, while a separate typed suppression
    /// origin keeps that failed layer terminal during diagnostic collection.
    /// Thus an erroneous request can continue lowering independent bodies
    /// without either exposing a partial target or falling through to a
    /// lower-priority declaration.
    pub(crate) fn finalize_import_targets(&mut self) {
        let unresolved = self
            .imports
            .bindings
            .iter()
            .enumerate()
            .filter_map(|(index, binding)| {
                let unresolved = match binding.target {
                    CurrentUnitTarget::SourceProperty(id) => {
                        !self.imports.resolved_properties.contains_key(&id)
                    }
                    CurrentUnitTarget::SourceVariant(id) => {
                        !self.imports.resolved_variants.contains_key(&id)
                    }
                    _ => false,
                };
                unresolved.then_some(CurrentUnitBindingId(index))
            })
            .collect::<std::collections::HashSet<_>>();
        if !unresolved.is_empty() {
            assert!(
                !self.diagnostics.is_empty(),
                "a successful declaration pass materializes the entire import surface"
            );
        }

        for binding in &mut self.imports.bindings {
            binding.target = match binding.target {
                CurrentUnitTarget::SourceProperty(id) => self
                    .imports
                    .resolved_properties
                    .get(&id)
                    .copied()
                    .map(CurrentUnitTarget::Property)
                    .unwrap_or(CurrentUnitTarget::SourceProperty(id)),
                CurrentUnitTarget::SourceVariant(id) => self
                    .imports
                    .resolved_variants
                    .get(&id)
                    .copied()
                    .map(CurrentUnitTarget::EnumVariant)
                    .unwrap_or(CurrentUnitTarget::SourceVariant(id)),
                target => target,
            };
        }

        let mut namespace_suppressions = Vec::new();
        for (namespace, members) in &mut self.imports.namespaces {
            members.retain(|name, bindings| {
                for binding in bindings.iter().copied() {
                    if unresolved.contains(&binding) {
                        namespace_suppressions.push((*namespace, name.clone(), binding));
                    }
                }
                bindings.retain(|binding| !unresolved.contains(binding));
                !bindings.is_empty()
            });
        }

        let mut exact_suppressions = Vec::new();
        let mut star_suppressions = Vec::new();
        for (file, imports) in self.imports.files.iter_mut().enumerate() {
            imports.exact.retain(|import| {
                let mut failed = false;
                for target in import.targets.iter() {
                    if let Some(binding) = target.current_binding()
                        && unresolved.contains(&binding)
                    {
                        exact_suppressions.push((file, import.local_name.clone(), binding));
                        failed = true;
                    }
                }
                !failed
            });
            for import in &mut imports.stars {
                import.snapshot.retain(|name, targets| {
                    let mut failed = false;
                    for target in targets.iter() {
                        if let Some(binding) = target.current_binding()
                            && unresolved.contains(&binding)
                        {
                            star_suppressions.push((file, name.clone(), binding));
                            failed = true;
                        }
                    }
                    !failed
                });
            }
        }

        for (namespace, name, binding) in namespace_suppressions {
            self.imports.diagnostic_suppressions.insert(
                SuppressedValueScope::Namespace(namespace),
                name,
                binding,
            );
        }
        for (file, name, binding) in exact_suppressions {
            self.imports.diagnostic_suppressions.insert(
                SuppressedValueScope::Exact { file },
                name,
                binding,
            );
        }
        for (file, name, binding) in star_suppressions {
            self.imports.diagnostic_suppressions.insert(
                SuppressedValueScope::Star { file },
                name,
                binding,
            );
        }

        self.imports
            .source_extension_properties
            .retain(|source| self.imports.resolved_properties.contains_key(source));
        self.imports.validate_frozen_scopes(self);
    }
}
