use super::*;

pub(super) struct InheritedProperty {
    pub(super) reference: hir::PropertyReference,
    ty: TypeId,
    mutable: bool,
    is_final: bool,
    slot: Option<hir::SlotContractDomain>,
}

impl Lowerer {
    pub(super) fn check_owner_properties(&mut self, owner: Owner) {
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
            for inherited in inherited {
                if !self.types_equal(declaration.ty, inherited.ty) {
                    self.error(
                        declaration.span,
                        format!(
                            "property `{}` must have the exact inherited type {}, found {}",
                            declaration.name,
                            self.type_name(inherited.ty),
                            self.type_name(declaration.ty)
                        ),
                    );
                    continue;
                }
                if inherited.mutable && declaration.capability.setter().is_none() {
                    self.error(
                        declaration.span,
                        format!(
                            "immutable property `{}` cannot override a mutable property",
                            declaration.name
                        ),
                    );
                    continue;
                }
                if inherited.is_final {
                    self.error(
                        declaration.span,
                        format!(
                            "property `{}` cannot override final property declared by {}",
                            declaration.name,
                            match inherited.reference {
                                hir::PropertyReference::Local(id) =>
                                    self.property_owner_name(self.properties[id].owner),
                                hir::PropertyReference::Imported { owner, .. } =>
                                    self.type_name(owner),
                            }
                        ),
                    );
                    continue;
                }
                valid_override = true;
                if !declaration.is_override {
                    continue;
                }
                self.check_property_override_access(property, inherited.slot.as_ref());
                if !self.properties[property]
                    .overrides
                    .contains(&inherited.reference)
                {
                    self.properties[property]
                        .overrides
                        .push(inherited.reference);
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

    pub(super) fn inherited_property_candidates(
        &mut self,
        owner: Owner,
        name: &str,
    ) -> Vec<InheritedProperty> {
        let receiver_ty = self.owner_ty(owner);
        let mut result = Vec::new();
        let class = match owner {
            Owner::Class(class) => Some(class),
            Owner::Object(object) => Some(self.objects[object].backing_class),
            _ => None,
        };
        if let Some(class) = class
            && let Some((property, ty)) = self.inherited_class_property(class, name)
        {
            result.push(self.local_inherited_property(property, ty));
        }
        for interface_ty in self.owner_interfaces(owner) {
            match self.types[interface_ty].clone() {
                Type::Interface(application) => {
                    let value = self.interface_applications[application].clone();
                    for property in self.interfaces[value.template].properties.clone() {
                        let declaration = &self.properties[property];
                        if declaration.name != name
                            || !self.property_is_accessible(property, Some(receiver_ty))
                        {
                            continue;
                        }
                        let ty =
                            self.instantiate_ty(self.properties[property].ty, &value.arguments);
                        let candidate = self.local_inherited_property(property, ty);
                        if !result.iter().any(|existing| {
                            existing.reference == candidate.reference
                                && self.types_equal(existing.ty, ty)
                        }) {
                            result.push(candidate);
                        }
                    }
                }
                Type::ImportedInterface(interface) => {
                    for method in &interface.methods {
                        if method.name != name
                            || method.slot.key().role()
                                != scoop_identity::DispatchRole::PropertyGetter
                        {
                            continue;
                        }
                        let scoop_identity::CallableTemplateOrigin::Accessor(accessor) =
                            method.declaration.declaration()
                        else {
                            unreachable!("a property slot references its actual accessor")
                        };
                        let property = self
                            .dependencies
                            .as_ref()
                            .and_then(|dependencies| dependencies.property_for_accessor(accessor))
                            .expect("an imported property accessor has a declaration");
                        let hir::PropertyDeclarationId::Property(declaration) =
                            property.declaration()
                        else {
                            unreachable!("interface properties are nominal declarations")
                        };
                        let hir::PublicDeclarationOwnerV1::Nominal(hir::SourceNominalId::Concrete(
                            owner,
                        )) = property.owner()
                        else {
                            unreachable!("interface properties have nominal owners")
                        };
                        let mutable = property.capability().setter().is_some();
                        let owner = self
                            .imported_signature_type(&scoop_identity::SignatureTypeKey::Nominal(
                                owner,
                            ))
                            .expect("the imported property owner is resolved with its interface");
                        let reference = hir::PropertyReference::Imported { owner, declaration };
                        if !result
                            .iter()
                            .any(|existing| existing.reference == reference)
                        {
                            result.push(InheritedProperty {
                                reference,
                                ty: method.return_type,
                                mutable,
                                is_final: method.declaration.modality()
                                    == hir::CallableModalityV1::Final,
                                slot: Some(hir::SlotContractDomain(hir::AccessDomain::universal())),
                            });
                        }
                    }
                }
                _ => unreachable!("the interface closure contains only interface types"),
            }
        }
        result
    }

    fn local_inherited_property(&self, property: hir::PropertyId, ty: TypeId) -> InheritedProperty {
        let declaration = &self.properties[property];
        InheritedProperty {
            reference: hir::PropertyReference::Local(property),
            ty,
            mutable: declaration.capability.setter().is_some(),
            is_final: declaration.modifier == hir::MethodModifier::Final,
            slot: declaration.access.slot.clone(),
        }
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
        inherited: Option<&hir::SlotContractDomain>,
    ) {
        let Some(mut provided) = self.properties[property].access.slot.clone() else {
            unreachable!("an overriding property owns a slot contract")
        };
        if self.properties[property].access.declared == hir::DeclaredVisibility::Protected
            && let Some(required) = inherited
        {
            provided = required.clone();
            self.properties[property].access.slot = Some(provided.clone());
        }
        let Some(required) = inherited else {
            return;
        };
        if !self.access_domain_is_subset(&required.0, &provided.0) {
            self.error(
                self.properties[property].span,
                format!(
                    "visibility of property `{}` does not cover its inherited slot",
                    self.properties[property].name
                ),
            );
        }
    }

    pub(super) fn property_has_callable_accessor(
        &self,
        property: hir::PropertyReference,
        kind: crate::properties::PropertyAccessorKind,
    ) -> bool {
        let hir::PropertyReference::Local(property) = property else {
            let hir::PropertyReference::Imported { declaration, .. } = property else {
                unreachable!("local properties were handled above")
            };
            let declaration = self
                .dependencies
                .as_ref()
                .and_then(|dependencies| {
                    dependencies
                        .property_declaration(hir::PropertyDeclarationId::Property(declaration))
                })
                .expect("an inherited property has its actual dependency declaration");
            return match kind {
                crate::properties::PropertyAccessorKind::Getter => true,
                crate::properties::PropertyAccessorKind::Setter => {
                    declaration.capability().setter().is_some()
                }
            };
        };
        let accessor = match kind {
            crate::properties::PropertyAccessorKind::Getter => {
                let getter = self.properties[property].capability.getter();
                self.property_getters[getter].implementation
            }
            crate::properties::PropertyAccessorKind::Setter => {
                let Some(setter) = self.properties[property].capability.setter() else {
                    return false;
                };
                self.property_setters[setter].implementation
            }
        };
        matches!(
            accessor,
            hir::PropertyAccessorImplementation::Body(_)
                | hir::PropertyAccessorImplementation::AbstractSlot(_)
        )
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
