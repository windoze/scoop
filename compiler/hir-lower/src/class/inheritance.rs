use super::*;
mod interfaces;
mod overrides;
mod signatures;

impl Lowerer {
    /// Inheritance checks (pass 2.75): cycles, property shadowing,
    /// `override` rules and interface implementation — for classes and
    /// for value types implementing interfaces (spec 4.4.3).
    pub(crate) fn check_inheritance(
        &mut self,
        pending_classes: &[(ClassId, &ast::ClassDecl, usize)],
        pending_structs: &[(hir::StructId, &ast::StructDecl, usize)],
        pending_enums: &[(hir::EnumId, &ast::EnumDecl, usize)],
        pending_objects: &[(hir::ObjectId, crate::declarations::ObjectSource<'_>, usize)],
        pending_methods: &[(FunctionId, &ast::FunctionDecl, usize, Owner)],
    ) {
        for &(id, decl, file_index) in pending_classes {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Class(id));
            self.check_inheritance_cycle(id, decl.span, "class");
        }
        for &(object, source, file_index) in pending_objects {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Object(object));
            self.check_inheritance_cycle(
                self.objects[object].backing_class,
                source.span(),
                source.description(),
            );
        }
        for (id, _) in self.interfaces.clone().iter() {
            self.current_file = self.interface_files[&id];
            self.current_owner = Some(Owner::Interface(id));
            self.check_owner_properties(Owner::Interface(id));
        }
        for &(id, _, file_index) in pending_classes {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Class(id));
            self.check_owner_properties(Owner::Class(id));
        }
        for &(id, _, file_index) in pending_objects {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Object(id));
            self.check_owner_properties(Owner::Object(id));
        }
        for &(id, _, file_index) in pending_structs {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Struct(id));
            self.check_owner_properties(Owner::Struct(id));
        }
        for &(id, _, file_index) in pending_enums {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Enum(id));
            self.check_owner_properties(Owner::Enum(id));
        }
        self.current_owner = None;
        for source in self.property_accessor_sources.clone() {
            let Some(owner) = source.owner else {
                continue;
            };
            self.current_file = self.function_files[&source.function];
            self.current_owner = Some(owner);
            let mut declaration = source.declaration.clone();
            let validated_target = self.properties[source.property]
                .overrides
                .iter()
                .copied()
                .any(|inherited| {
                    self.property_accessor_function(inherited, source.kind)
                        .is_some()
                });
            let declared_target = if source.declaration.is_override {
                let name = self.properties[source.property].name.clone();
                self.inherited_property_candidates(owner, &name)
                    .into_iter()
                    .any(|(inherited, _)| {
                        self.property_accessor_function(inherited, source.kind)
                            .is_some()
                    })
            } else {
                false
            };
            declaration.is_override = validated_target || declared_target;
            self.check_member_access_contract(source.function, &declaration, owner);
            self.check_override_rules(source.function, &declaration, owner);
        }
        self.current_owner = None;
        for &(id, decl, file_index, owner) in pending_methods {
            self.current_file = file_index;
            self.current_owner = Some(owner);
            self.check_member_access_contract(id, decl, owner);
            self.check_override_rules(id, decl, owner);
        }
        self.current_owner = None;
        for &(id, decl, file_index) in pending_classes {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Class(id));
            self.check_interface_implementation(
                id,
                decl.span,
                &format!("class `{}`", decl.name.text),
            );
        }
        for &(object, source, file_index) in pending_objects {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Object(object));
            self.check_interface_implementation(
                self.objects[object].backing_class,
                source.span(),
                &format!("{} `{}`", source.description(), self.objects[object].name),
            );
        }
        for &(id, decl, file_index) in pending_structs {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Struct(id));
            self.check_value_interface_implementation(Owner::Struct(id), decl.span);
        }
        for &(id, decl, file_index) in pending_enums {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Enum(id));
            self.check_value_interface_implementation(Owner::Enum(id), decl.span);
        }
        self.current_owner = None;
    }

    /// A class may not directly or indirectly inherit from itself.
    fn check_inheritance_cycle(&mut self, id: ClassId, span: ast::Span, host: &str) {
        let mut seen = vec![id];
        let mut current = id;
        while let Some(base_ty) = self.classes[current].base_class {
            let Type::Class(base_application) = self.types[base_ty] else {
                unreachable!("resolved class bases are class applications")
            };
            let base = self.class_applications[base_application].template;
            if seen.contains(&base) {
                let name = self.classes[id].name.clone();
                self.error(
                    span,
                    format!("{host} `{name}` directly or indirectly inherits from itself"),
                );
                return;
            }
            seen.push(base);
            current = base;
        }
    }

    fn check_owner_properties(&mut self, owner: Owner) {
        let properties = match owner {
            Owner::Class(id) => self.classes[id].properties.clone(),
            Owner::Struct(id) => self.structs[id].properties.clone(),
            Owner::Enum(id) => self.enums[id].properties.clone(),
            Owner::Interface(id) => self.interfaces[id].properties.clone(),
            Owner::Object(id) => self.classes[self.objects[id].backing_class]
                .properties
                .clone(),
        };
        for property in properties {
            let declaration = self.properties[property].clone();
            if matches!(owner, Owner::Class(_) | Owner::Object(_))
                && declaration.access.declared == hir::DeclaredVisibility::Private
                && declaration.modifier != hir::MethodModifier::Final
            {
                self.error(
                    declaration.span,
                    format!("private property `{}` must be final", declaration.name),
                );
            }
            if declaration.modifier == hir::MethodModifier::Abstract
                && let Owner::Class(class) = owner
            {
                let required = self.classes[class].access.inheritance.0.clone();
                let Some(provided) = declaration.access.slot.as_ref() else {
                    unreachable!("an abstract property owns a slot contract")
                };
                if !self.access_domain_is_subset(&required, &provided.0) {
                    self.error(
                        declaration.span,
                        format!(
                            "abstract property `{}` is not visible throughout the inheritance domain of class `{}`",
                            declaration.name, self.classes[class].name
                        ),
                    );
                }
            }

            if declaration.access.declared == hir::DeclaredVisibility::Private {
                continue;
            }
            let inherited = self.inherited_property_candidates(owner, &declaration.name);
            if inherited.is_empty() {
                if declaration.is_override {
                    self.error(
                        declaration.span,
                        format!(
                            "property `{}` is marked `override` but does not override any property",
                            declaration.name
                        ),
                    );
                }
                continue;
            }
            let mut valid_override = false;
            for (inherited, inherited_ty) in inherited {
                let inherited_declaration = self.properties[inherited].clone();
                if !self.types_equal(declaration.ty, inherited_ty) {
                    self.error(
                        declaration.span,
                        format!(
                            "property `{}` must have the exact inherited type {}, found {}",
                            declaration.name,
                            self.type_name(inherited_ty),
                            self.type_name(declaration.ty)
                        ),
                    );
                    continue;
                }
                if inherited_declaration.capability.setter().is_some()
                    && declaration.capability.setter().is_none()
                {
                    self.error(
                        declaration.span,
                        format!(
                            "immutable property `{}` cannot override a mutable property",
                            declaration.name
                        ),
                    );
                    continue;
                }
                if inherited_declaration.modifier == hir::MethodModifier::Final {
                    self.error(
                        declaration.span,
                        format!(
                            "property `{}` cannot override final property declared by {}",
                            declaration.name,
                            self.property_owner_name(inherited_declaration.owner)
                        ),
                    );
                    continue;
                }
                valid_override = true;
                if let Some(witness) = self.check_property_override_access(property, inherited) {
                    self.properties[property].override_access.push(witness);
                }
                if !self.properties[property].overrides.contains(&inherited) {
                    self.properties[property].overrides.push(inherited);
                }
            }
            if valid_override && !declaration.is_override {
                self.error(
                    declaration.span,
                    format!(
                        "property `{}` overrides an inherited property and must be marked `override`",
                        declaration.name
                    ),
                );
            }
        }
    }

    fn inherited_property_candidates(
        &mut self,
        owner: Owner,
        name: &str,
    ) -> Vec<(hir::PropertyId, TypeId)> {
        let receiver_ty = self.owner_ty(owner);
        let mut result = Vec::new();
        if let Owner::Class(class) = owner
            && let Some(candidate) = self.inherited_class_property(class, name)
        {
            result.push(candidate);
        }
        if let Owner::Object(object) = owner
            && let Some(candidate) =
                self.inherited_class_property(self.objects[object].backing_class, name)
        {
            result.push(candidate);
        }
        let roots = match owner {
            Owner::Class(class) => self.class_interfaces_all(class),
            Owner::Struct(id) => self.structs[id].interfaces.clone(),
            Owner::Enum(id) => self.enums[id].interfaces.clone(),
            Owner::Interface(id) => self.interfaces[id]
                .parents
                .iter()
                .map(|parent| self.interface_applications[*parent].canonical_type)
                .collect(),
            Owner::Object(id) => self.class_interfaces_all(self.objects[id].backing_class),
        };
        let mut interfaces = Vec::new();
        for interface in roots {
            self.append_interface_closure(interface, &mut interfaces);
        }
        for interface_ty in interfaces {
            let Type::Interface(application) = self.types[interface_ty] else {
                unreachable!("interface property candidates are interface applications")
            };
            let value = self.interface_applications[application].clone();
            for property in self.interfaces[value.template].properties.clone() {
                let declaration = self.properties[property].clone();
                if declaration.name != name
                    || !self.access_domain_allows(&declaration.access.lookup.0, Some(receiver_ty))
                {
                    continue;
                }
                let ty = self.instantiate_ty(declaration.ty, &value.arguments);
                if !result.iter().any(|(existing, existing_ty)| {
                    *existing == property && self.types_equal(*existing_ty, ty)
                }) {
                    result.push((property, ty));
                }
            }
        }
        result
    }

    fn inherited_class_property(
        &mut self,
        class: ClassId,
        name: &str,
    ) -> Option<(hir::PropertyId, TypeId)> {
        let receiver_ty =
            self.class_applications[self.classes[class].self_application].canonical_type;
        let current = self.classes[class].self_application;
        let application = self.class_applications[current].clone();
        let base = self.classes[class].base_class?;
        let base = self.instantiate_ty(base, &application.arguments);
        let Type::Class(base_application) = self.types[base] else {
            unreachable!("resolved class bases are class applications")
        };
        let (_, property, ty) =
            self.find_accessible_class_application_property(base_application, name, receiver_ty)?;
        Some((property, ty))
    }

    fn check_property_override_access(
        &mut self,
        property: hir::PropertyId,
        inherited: hir::PropertyId,
    ) -> Option<hir::PropertyOverrideAccessWitness> {
        let Some(mut provided) = self.properties[property].access.slot.clone() else {
            unreachable!("an overriding property owns a slot contract")
        };
        if self.properties[property].access.declared == hir::DeclaredVisibility::Protected
            && let Some(required) = self.properties[inherited].access.slot.clone()
        {
            provided = required;
            self.properties[property].access.slot = Some(provided.clone());
        }
        let required = self.properties[inherited].access.slot.clone()?;
        if !self.access_domain_is_subset(&required.0, &provided.0) {
            self.error(
                self.properties[property].span,
                format!(
                    "visibility of property `{}` does not cover its inherited slot",
                    self.properties[property].name
                ),
            );
            return None;
        }
        Some(hir::PropertyOverrideAccessWitness {
            overriding: property,
            inherited,
            required,
            provided,
        })
    }

    fn property_accessor_function(
        &self,
        property: hir::PropertyId,
        kind: crate::properties::PropertyAccessorKind,
    ) -> Option<FunctionId> {
        let accessor = match kind {
            crate::properties::PropertyAccessorKind::Getter => {
                let getter = self.properties[property].capability.getter();
                self.property_getters[getter].implementation
            }
            crate::properties::PropertyAccessorKind::Setter => {
                let setter = self.properties[property].capability.setter()?;
                self.property_setters[setter].implementation
            }
        };
        match accessor {
            hir::PropertyAccessorImplementation::Body(function)
            | hir::PropertyAccessorImplementation::AbstractSlot(function) => Some(function),
            hir::PropertyAccessorImplementation::Storage
            | hir::PropertyAccessorImplementation::Constant => None,
        }
    }

    fn property_owner_name(&self, owner: hir::PropertyOwner) -> String {
        match owner {
            hir::PropertyOwner::Class(owner) => format!("class `{}`", self.classes[owner].name),
            hir::PropertyOwner::Struct(owner) => format!("struct `{}`", self.structs[owner].name),
            hir::PropertyOwner::Enum(owner) => format!("enum `{}`", self.enums[owner].name),
            hir::PropertyOwner::Interface(owner) => {
                format!("interface `{}`", self.interfaces[owner].name)
            }
            hir::PropertyOwner::Object(owner) => {
                format!("object `{}`", self.objects[owner].name)
            }
            hir::PropertyOwner::Extension(_) => "an extension receiver".to_string(),
            hir::PropertyOwner::TopLevel => "top level".to_string(),
        }
    }
}
