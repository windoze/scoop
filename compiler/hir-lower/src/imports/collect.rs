use super::*;
use crate::{
    Lowerer, NominalTarget, Owner, SourceKind,
    namespace::{TopLevelLookupLayer, TopLevelTypeTarget},
};

impl Lowerer {
    fn import_source(&self, file: usize) -> ast::Stage1SourceHandle {
        match self.visibility_file(file).source {
            hir::VisibilitySource::CurrentUnit(handle) => handle,
            hir::VisibilitySource::ExistingM22Core { .. } => {
                unreachable!("core has no current-unit import surface")
            }
        }
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
        functions: &[(hir::FunctionId, &ast::FunctionDecl, usize)],
        methods: &[(hir::FunctionId, &ast::FunctionDecl, usize, Owner)],
        properties: &[(&ast::GlobalDecl, usize)],
        enumerations: &[(hir::EnumId, &ast::EnumDecl, usize)],
        objects: &[(hir::ObjectId, crate::declarations::ObjectSource<'_>, usize)],
    ) {
        let mut surface = CurrentUnitImports::default();
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
                surface.resolve_file(self, syntax)
            } else {
                FrozenFileImports::default()
            };
            surface.files.push(imports);
        }
        self.imports = surface;
    }

    /// Resolve source-only property/variant ids before any successful body
    /// lookup can consume the frozen import membership.
    pub(crate) fn finalize_import_targets(&mut self) -> bool {
        let complete = self.imports.resolved_properties.len() == self.imports.source_property_count
            && self.imports.source_variants.iter().all(|source| {
                hir::EnumVariantRef::checked(&self.enums, source.enumeration, source.index)
                    .is_some()
            });
        if !complete {
            assert!(
                !self.diagnostics.is_empty(),
                "a successful declaration pass materializes the entire import surface"
            );
            return false;
        }
        for binding in &mut self.imports.bindings {
            binding.target = match binding.target {
                CurrentUnitTarget::SourceProperty(id) => {
                    CurrentUnitTarget::Property(self.imports.resolved_properties[&id])
                }
                CurrentUnitTarget::SourceVariant(id) => {
                    let source = &self.imports.source_variants[id.0];
                    CurrentUnitTarget::EnumVariant(
                        hir::EnumVariantRef::checked(&self.enums, source.enumeration, source.index)
                            .expect("successful enum lowering retains every source variant"),
                    )
                }
                target => target,
            };
        }
        self.imports.validate_frozen_scopes(self);
        true
    }
}
