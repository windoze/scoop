//! Export surface assembly (DESIGN 4.3). Local compilation still
//! consumes the whole module; these typed root sets are what the T19
//! packager serializes, and the binding index is the only name-keyed
//! surface — hidden support and inheritance-only entities carry no
//! entry.

use std::collections::{HashMap, HashSet};

use scoop_hir as hir;

use hir::DeclaredVisibility;

use crate::{Lowerer, Type};

impl Lowerer {
    /// Assembles the six DESIGN 4.3 surfaces from the finished
    /// declaration set.
    pub(crate) fn export_surfaces(&self) -> hir::ExportSurfaces {
        let object_backings = self
            .objects
            .iter()
            .map(|(_, declaration)| declaration.backing_class)
            .collect::<HashSet<_>>();
        let accessor_functions = self
            .property_accessor_sources
            .iter()
            .map(|source| source.function)
            .collect::<HashSet<_>>();
        let is_exported = |access: &hir::DeclarationAccess| Self::declaration_is_exported(access);
        let nominal_exported = |access: &hir::NominalAccess| Self::nominal_is_exported(access);

        // Surface 1: public lookup.
        let functions: Vec<hir::FunctionId> = self
            .functions
            .iter()
            .filter_map(|(id, function)| {
                (!accessor_functions.contains(&id) && is_exported(&function.access)).then_some(id)
            })
            .collect();
        let properties: Vec<hir::PropertyId> = self
            .properties
            .iter()
            .filter_map(|(id, property)| is_exported(&property.access).then_some(id))
            .collect();
        let structs: Vec<hir::StructId> = self
            .structs
            .iter()
            .filter_map(|(id, declaration)| nominal_exported(&declaration.access).then_some(id))
            .collect();
        let enums: Vec<hir::EnumId> = self
            .enums
            .iter()
            .filter_map(|(id, declaration)| nominal_exported(&declaration.access).then_some(id))
            .collect();
        let classes: Vec<hir::ClassId> = self
            .classes
            .iter()
            .filter_map(|(id, declaration)| {
                (!object_backings.contains(&id) && nominal_exported(&declaration.access))
                    .then_some(id)
            })
            .collect();
        let interfaces: Vec<hir::InterfaceId> = self
            .interfaces
            .iter()
            .filter_map(|(id, declaration)| nominal_exported(&declaration.access).then_some(id))
            .collect();
        let objects: Vec<hir::ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, declaration)| nominal_exported(&declaration.access).then_some(id))
            .collect();
        let generic_functions: Vec<hir::GenericFunctionId> = self
            .generic_functions
            .iter()
            .filter_map(|(id, generic)| functions.contains(&generic.function).then_some(id))
            .collect();
        let generic_methods: Vec<hir::GenericMethodId> = self
            .generic_methods
            .iter()
            .filter_map(|(id, generic)| functions.contains(&generic.function).then_some(id))
            .collect();
        let public_lookup = hir::PublicLookupSurface {
            functions: functions.clone(),
            properties: properties.clone(),
            property_getters: self
                .property_getters
                .iter()
                .filter_map(|(id, getter)| is_exported(&getter.access).then_some(id))
                .collect(),
            property_setters: self
                .property_setters
                .iter()
                .filter_map(|(id, setter)| is_exported(&setter.access).then_some(id))
                .collect(),
            generic_functions: generic_functions.clone(),
            generic_methods: generic_methods.clone(),
            structs: structs.clone(),
            struct_constructors: self
                .struct_constructors
                .iter()
                .filter_map(|(id, ctor)| is_exported(&ctor.access).then_some(id))
                .collect(),
            enums: enums.clone(),
            classes: classes.clone(),
            class_constructors: self
                .class_constructors
                .iter()
                .filter_map(|(id, ctor)| {
                    (!object_backings.contains(&ctor.owner) && is_exported(&ctor.access))
                        .then_some(id)
                })
                .collect(),
            interfaces: interfaces.clone(),
            interface_methods: self
                .interface_method_entities
                .iter()
                .filter_map(|(id, method)| functions.contains(&method.function).then_some(id))
                .collect(),
            objects: objects.clone(),
            object_types: self
                .objects
                .iter()
                .filter_map(|(_, declaration)| {
                    nominal_exported(&declaration.access).then_some(declaration.object_type)
                })
                .collect(),
            companion_relations: self
                .objects
                .iter()
                .filter_map(|(_, declaration)| match declaration.kind {
                    hir::ObjectKind::Companion(relation)
                        if nominal_exported(&declaration.access) =>
                    {
                        Some(relation)
                    }
                    _ => None,
                })
                .collect(),
            singleton_values: self
                .objects
                .iter()
                .filter_map(|(_, declaration)| {
                    nominal_exported(&declaration.access).then_some(declaration.singleton_value)
                })
                .collect(),
            type_aliases: self
                .type_aliases
                .iter()
                .filter_map(|(id, alias)| is_exported(&alias.access).then_some(id))
                .collect(),
            reexports: self.reexports.clone(),
        };

        // Surface 2: protected inheritance contracts of publicly
        // inheritable owners.
        let is_protected =
            |access: &hir::DeclarationAccess| access.declared == DeclaredVisibility::Protected;
        let mut protected_methods = Vec::new();
        let mut protected_properties = Vec::new();
        let mut open_class_ids = Vec::new();
        for class in &classes {
            if !matches!(
                self.classes[*class].modifier,
                hir::ClassModifier::Open | hir::ClassModifier::Abstract
            ) {
                continue;
            }
            open_class_ids.push(*class);
            for method in &self.classes[*class].methods {
                if is_protected(&self.functions[*method].access) {
                    protected_methods.push((hir::InheritanceHost::Class(*class), *method));
                }
            }
            for property in &self.classes[*class].properties {
                if is_protected(&self.properties[*property].access) {
                    protected_properties.push((hir::InheritanceHost::Class(*class), *property));
                }
            }
        }
        let mut interface_method_pairs = Vec::new();
        for interface in &interfaces {
            for method in &self.interfaces[*interface].methods {
                interface_method_pairs.push((*interface, *method));
            }
        }
        let inheritance = hir::InheritanceSurface {
            open_classes: open_class_ids,
            interfaces: interfaces.clone(),
            protected_class_constructors: self
                .class_constructors
                .iter()
                .filter_map(|(id, ctor)| {
                    (is_protected(&ctor.access)
                        && !object_backings.contains(&ctor.owner)
                        && matches!(
                            self.classes[ctor.owner].modifier,
                            hir::ClassModifier::Open | hir::ClassModifier::Abstract
                        ))
                    .then_some((ctor.owner, id))
                })
                .collect(),
            protected_methods,
            protected_properties,
            interface_methods: interface_method_pairs,
        };

        // Surface 3: public generic templates.
        let template_support = hir::TemplateSupportClosure {
            generic_structs: structs
                .iter()
                .copied()
                .filter(|id| !self.structs[*id].type_params.is_empty())
                .collect(),
            generic_enums: enums
                .iter()
                .copied()
                .filter(|id| !self.enums[*id].type_params.is_empty())
                .collect(),
            generic_classes: classes
                .iter()
                .copied()
                .filter(|id| !self.classes[*id].type_params.is_empty())
                .collect(),
            generic_interfaces: interfaces
                .iter()
                .copied()
                .filter(|id| !self.interfaces[*id].type_params.is_empty())
                .collect(),
            generic_functions: generic_functions.clone(),
            generic_methods: generic_methods.clone(),
        };

        // Surface 4: nominal/interface entities reachable from the
        // public signatures, transitively through the type graph.
        let mut closure_types = Vec::new();
        for function in &functions {
            let signature = self.signatures.get(function);
            if let Some(signature) = signature {
                closure_types.extend(signature.params.iter().map(|param| param.ty));
                closure_types.push(signature.return_ty);
            }
        }
        for property in &properties {
            closure_types.push(self.properties[*property].ty);
        }
        let mut reached = HashSet::new();
        for ty in closure_types {
            self.collect_nominal_reach(ty, &mut reached);
        }
        let mut interface_dependency = hir::InterfaceDependencyClosure::default();
        for ty in &reached {
            match &self.types[*ty] {
                Type::Struct(application) => interface_dependency
                    .structs
                    .push(self.struct_applications[*application].template),
                Type::Enum(application) => interface_dependency
                    .enums
                    .push(self.enum_applications[*application].template),
                Type::Class(application) => interface_dependency
                    .classes
                    .push(self.class_applications[*application].template),
                Type::Interface(application) => interface_dependency
                    .interfaces
                    .push(self.interface_applications[*application].template),
                _ => {}
            }
        }
        interface_dependency
            .structs
            .sort_by_key(|id| id.into_raw().into_u32());
        interface_dependency
            .enums
            .sort_by_key(|id| id.into_raw().into_u32());
        interface_dependency
            .classes
            .sort_by_key(|id| id.into_raw().into_u32());
        interface_dependency
            .interfaces
            .sort_by_key(|id| id.into_raw().into_u32());
        interface_dependency.structs.dedup();
        interface_dependency.enums.dedup();
        interface_dependency.classes.dedup();
        interface_dependency.interfaces.dedup();
        interface_dependency.const_properties = properties
            .iter()
            .copied()
            .filter(|property| {
                matches!(
                    self.properties[*property].representation,
                    hir::PropertyRepresentation::Const { .. }
                )
            })
            .collect();

        // Surface 5: M17 source-parameter interfaces and defaults.
        let source_interface_templates = hir::SourceInterfaceTemplates {
            interfaces: (0..self.source_parameter_interfaces.len()).collect(),
            default_exprs: self.export_default_exprs.iter().map(|(id, _)| id).collect(),
        };

        // Surface 6: the name-keyed binding index.
        let binding_index = self.binding_index(
            &functions,
            &properties,
            &structs,
            &enums,
            &classes,
            &interfaces,
            &objects,
            &public_lookup.type_aliases,
        );

        // Wire purposes per entity definition.
        let mut purposes = HashMap::new();
        let mut mark = |entity: hir::ExportEntity, set: fn(&mut hir::ExportPurposes)| {
            let entry = purposes.entry(entity).or_default();
            set(entry);
        };
        for function in &functions {
            mark(hir::ExportEntity::Function(*function), |p| {
                p.public_lookup = true
            });
        }
        for property in &properties {
            mark(hir::ExportEntity::Property(*property), |p| {
                p.public_lookup = true
            });
        }
        for id in &structs {
            mark(hir::ExportEntity::Struct(*id), |p| p.public_lookup = true);
        }
        for id in &enums {
            mark(hir::ExportEntity::Enum(*id), |p| p.public_lookup = true);
        }
        for id in &classes {
            mark(hir::ExportEntity::Class(*id), |p| p.public_lookup = true);
        }
        for id in &interfaces {
            mark(hir::ExportEntity::Interface(*id), |p| {
                p.public_lookup = true
            });
        }
        for id in &objects {
            mark(hir::ExportEntity::Object(*id), |p| p.public_lookup = true);
        }
        for alias in &public_lookup.type_aliases {
            mark(hir::ExportEntity::TypeAlias(*alias), |p| {
                p.public_lookup = true
            });
        }
        for class in &inheritance.open_classes {
            mark(hir::ExportEntity::Class(*class), |p| p.inheritance = true);
        }
        for interface in &interfaces {
            mark(hir::ExportEntity::Interface(*interface), |p| {
                p.inheritance = true
            });
        }
        for generic in &generic_functions {
            mark(hir::ExportEntity::GenericFunction(*generic), |p| {
                p.template_support = true
            });
        }
        for template in &template_support.generic_structs {
            mark(hir::ExportEntity::Struct(*template), |p| {
                p.template_support = true
            });
        }
        for class in &interface_dependency.classes {
            mark(hir::ExportEntity::Class(*class), |p| {
                p.interface_dependency = true
            });
        }

        hir::ExportSurfaces {
            public_lookup,
            inheritance,
            template_support,
            interface_dependency,
            source_interface_templates,
            binding_index,
            purposes,
        }
    }

    /// Recursively collects the nominal applications reachable from a
    /// signature type.
    fn collect_nominal_reach(&self, ty: crate::TypeId, reached: &mut HashSet<crate::TypeId>) {
        if !reached.insert(ty) {
            return;
        }
        let arguments = match &self.types[ty] {
            Type::Struct(application) => self.struct_applications[*application].arguments.clone(),
            Type::Enum(application) => self.enum_applications[*application].arguments.clone(),
            Type::Class(application) => self.class_applications[*application].arguments.clone(),
            Type::Interface(application) => {
                self.interface_applications[*application].arguments.clone()
            }
            Type::Ptr(pointee) => vec![*pointee],
            Type::FunPtr(function) => self.function_signature_arguments(*function),
            Type::Function(function) => self.function_signature_arguments(*function),
            _ => Vec::new(),
        };
        for argument in arguments {
            self.collect_nominal_reach(argument, reached);
        }
    }

    fn function_signature_arguments(&self, function: hir::FunctionTypeId) -> Vec<crate::TypeId> {
        let signature = &self.function_types[function];
        let mut arguments = signature.parameter_types.clone();
        arguments.push(signature.return_type);
        arguments
    }

    /// The declaring file of one property through its owner; total
    /// where the source-declaration registry is not (interface and
    /// derived properties never register a file entry).
    fn property_declaring_file(&self, property: hir::PropertyId) -> usize {
        if let Some(file) = self.property_files.get(&property) {
            return *file;
        }
        match self.properties[property].owner {
            hir::PropertyOwner::TopLevel => self
                .globals
                .iter()
                .find(|(_, global)| global.property == property)
                .map(|(_, global)| global.origin.file as usize)
                .unwrap_or(0),
            hir::PropertyOwner::Class(id) => self.classes[id].origin.file as usize,
            hir::PropertyOwner::Struct(id) => self.structs[id].origin.file as usize,
            hir::PropertyOwner::Enum(id) => self.enums[id].origin.file as usize,
            hir::PropertyOwner::Interface(id) => self.interfaces[id].origin.file as usize,
            hir::PropertyOwner::Object(id) => {
                self.classes[self.objects[id].backing_class].origin.file as usize
            }
            hir::PropertyOwner::Extension(_) => self
                .property_accessor_sources
                .iter()
                .find(|source| source.property == property)
                .map(|source| self.functions[source.function].origin.file as usize)
                .unwrap_or(0),
        }
    }

    /// Builds the package/name/namespace binding index from public
    /// declarations and validated re-export bindings.
    #[allow(clippy::too_many_arguments)]
    fn binding_index(
        &self,
        functions: &[hir::FunctionId],
        properties: &[hir::PropertyId],
        structs: &[hir::StructId],
        enums: &[hir::EnumId],
        classes: &[hir::ClassId],
        interfaces: &[hir::InterfaceId],
        objects: &[hir::ObjectId],
        aliases: &[hir::ExportTypeAliasId],
    ) -> hir::BindingIndex {
        fn push(
            entries: &mut Vec<hir::BindingEntry>,
            package: hir::PackageId,
            name: &str,
            namespace: hir::BindingNamespace,
            root: hir::ExportEntity,
            provenance: hir::BindingProvenance,
        ) {
            if let Some(entry) = entries.iter_mut().find(|entry| {
                entry.package == package && entry.name == name && entry.namespace == namespace
            }) {
                if !entry.roots.contains(&root) {
                    entry.roots.push(root);
                }
                if provenance == hir::BindingProvenance::PublicDeclaration {
                    entry.provenance = provenance;
                }
                return;
            }
            entries.push(hir::BindingEntry {
                package,
                name: name.to_owned(),
                namespace,
                roots: vec![root],
                provenance,
            });
        }
        let mut entries: Vec<hir::BindingEntry> = Vec::new();
        let package_of = |file: u32| self.file_packages[file as usize];
        for function in functions {
            let function_decl = &self.functions[*function];
            push(
                &mut entries,
                package_of(function_decl.origin.file),
                &function_decl.name,
                hir::BindingNamespace::Value,
                hir::ExportEntity::Function(*function),
                hir::BindingProvenance::PublicDeclaration,
            );
        }
        for property in properties {
            let property_decl = &self.properties[*property];
            let file = self.property_declaring_file(*property);
            push(
                &mut entries,
                self.file_packages[file],
                &property_decl.name,
                hir::BindingNamespace::Value,
                hir::ExportEntity::Property(*property),
                hir::BindingProvenance::PublicDeclaration,
            );
        }
        for id in structs {
            let declaration = &self.structs[*id];
            push(
                &mut entries,
                package_of(declaration.origin.file),
                &declaration.name,
                hir::BindingNamespace::Type,
                hir::ExportEntity::Struct(*id),
                hir::BindingProvenance::PublicDeclaration,
            );
        }
        for id in enums {
            let declaration = &self.enums[*id];
            push(
                &mut entries,
                package_of(declaration.origin.file),
                &declaration.name,
                hir::BindingNamespace::Type,
                hir::ExportEntity::Enum(*id),
                hir::BindingProvenance::PublicDeclaration,
            );
        }
        for id in classes {
            let declaration = &self.classes[*id];
            push(
                &mut entries,
                package_of(declaration.origin.file),
                &declaration.name,
                hir::BindingNamespace::Type,
                hir::ExportEntity::Class(*id),
                hir::BindingProvenance::PublicDeclaration,
            );
        }
        for id in interfaces {
            let declaration = &self.interfaces[*id];
            push(
                &mut entries,
                package_of(declaration.origin.file),
                &declaration.name,
                hir::BindingNamespace::Type,
                hir::ExportEntity::Interface(*id),
                hir::BindingProvenance::PublicDeclaration,
            );
        }
        for id in objects {
            let declaration = &self.objects[*id];
            push(
                &mut entries,
                package_of(self.classes[declaration.backing_class].origin.file),
                &declaration.name,
                hir::BindingNamespace::Type,
                hir::ExportEntity::Object(*id),
                hir::BindingProvenance::PublicDeclaration,
            );
        }
        for alias in aliases {
            let declaration = &self.type_aliases[*alias];
            push(
                &mut entries,
                package_of(declaration.origin.file),
                &declaration.name,
                hir::BindingNamespace::Type,
                hir::ExportEntity::TypeAlias(*alias),
                hir::BindingProvenance::PublicDeclaration,
            );
        }
        // Re-export bindings publish under their destination package.
        for reexport in &self.reexports {
            for target in &reexport.binding.targets {
                let (root, namespace) = match &target.target {
                    hir::ImportedTarget::Function { function } => (
                        hir::ExportEntity::Function(*function),
                        hir::BindingNamespace::Value,
                    ),
                    hir::ImportedTarget::Property { property } => (
                        hir::ExportEntity::Property(*property),
                        hir::BindingNamespace::Value,
                    ),
                    hir::ImportedTarget::Struct { declaration } => (
                        hir::ExportEntity::Struct(*declaration),
                        hir::BindingNamespace::Type,
                    ),
                    hir::ImportedTarget::Enum { declaration } => (
                        hir::ExportEntity::Enum(*declaration),
                        hir::BindingNamespace::Type,
                    ),
                    hir::ImportedTarget::Class { declaration } => (
                        hir::ExportEntity::Class(*declaration),
                        hir::BindingNamespace::Type,
                    ),
                    hir::ImportedTarget::Interface { declaration } => (
                        hir::ExportEntity::Interface(*declaration),
                        hir::BindingNamespace::Type,
                    ),
                    hir::ImportedTarget::Object { declaration } => (
                        hir::ExportEntity::Object(*declaration),
                        hir::BindingNamespace::Type,
                    ),
                    hir::ImportedTarget::TypeAlias { alias } => (
                        hir::ExportEntity::TypeAlias(*alias),
                        hir::BindingNamespace::Type,
                    ),
                    hir::ImportedTarget::Variant { variant } => (
                        hir::ExportEntity::Variant(*variant),
                        hir::BindingNamespace::Value,
                    ),
                };
                push(
                    &mut entries,
                    reexport.package,
                    &reexport.name,
                    namespace,
                    root,
                    hir::BindingProvenance::ReExport,
                );
            }
        }
        entries.sort_by(|a, b| {
            (u32::from(a.package.into_raw()), a.name.clone(), a.namespace).cmp(&(
                u32::from(b.package.into_raw()),
                b.name.clone(),
                b.namespace,
            ))
        });
        hir::BindingIndex { entries }
    }
}
