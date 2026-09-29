use super::*;

mod declaration;

#[derive(Clone, Copy)]
pub(crate) enum ObjectSource<'a> {
    Object(&'a ast::ObjectDecl),
    Companion(&'a ast::CompanionObjectDecl),
}

impl<'a> ObjectSource<'a> {
    pub(crate) fn name(self) -> (&'a str, ast::Span) {
        match self {
            Self::Object(declaration) => (&declaration.name.text, declaration.name.span),
            Self::Companion(declaration) => match &declaration.name {
                ast::CompanionNameSyntax::Default { span } => ("Companion", *span),
                ast::CompanionNameSyntax::Named(name) => (&name.text, name.span),
            },
        }
    }

    pub(crate) fn members(self) -> &'a [ast::ClassMember] {
        match self {
            Self::Object(declaration) => &declaration.members,
            Self::Companion(declaration) => &declaration.members,
        }
    }

    pub(crate) fn supertypes(self) -> &'a [ast::SupertypeSpec] {
        match self {
            Self::Object(declaration) => &declaration.supertypes,
            Self::Companion(declaration) => &declaration.supertypes,
        }
    }

    pub(crate) fn span(self) -> ast::Span {
        match self {
            Self::Object(declaration) => declaration.span,
            Self::Companion(declaration) => declaration.span,
        }
    }

    fn annotations(self) -> &'a [ast::Annotation] {
        match self {
            Self::Object(declaration) => &declaration.annotations,
            Self::Companion(declaration) => &declaration.annotations,
        }
    }

    fn visibility(self) -> ast::VisibilitySyntax {
        match self {
            Self::Object(declaration) => declaration.visibility,
            Self::Companion(declaration) => declaration.visibility,
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Object(_) => "object",
            Self::Companion(_) => "companion object",
        }
    }

    pub(crate) fn article_description(self) -> &'static str {
        match self {
            Self::Object(_) => "an object",
            Self::Companion(_) => "a companion object",
        }
    }

    fn companion_name(self) -> Option<hir::CompanionName> {
        match self {
            Self::Object(_) => None,
            Self::Companion(declaration) => Some(match &declaration.name {
                ast::CompanionNameSyntax::Default { .. } => hir::CompanionName::Default,
                ast::CompanionNameSyntax::Named(name) => {
                    hir::CompanionName::Named(name.text.clone())
                }
            }),
        }
    }
}

impl Lowerer {
    pub(crate) fn companion_object(&self, host: Owner) -> Option<ObjectId> {
        let relation = *self.companion_by_host.get(&host)?;
        Some(self.companion_relations[relation].object)
    }

    pub(crate) fn companion_host(&self, object: ObjectId) -> Option<Owner> {
        let hir::ObjectKind::Companion(relation) = self.objects[object].kind else {
            return None;
        };
        Some(Owner::from_nominal_owner(
            self.companion_relations[relation].host,
        ))
    }

    pub(crate) fn current_owner_is_companion(&self) -> bool {
        matches!(
            self.current_owner,
            Some(Owner::Object(object)) if self.companion_host(object).is_some()
        )
    }

    pub(crate) fn companion_host_declares_property(&self, object: ObjectId, name: &str) -> bool {
        let Some(host) = self.companion_host(object) else {
            return false;
        };
        let properties = match host {
            Owner::Class(class) => &self.classes[class].properties,
            Owner::Interface(interface) => &self.interfaces[interface].properties,
            Owner::Struct(structure) => &self.structs[structure].properties,
            Owner::Enum(enumeration) => &self.enums[enumeration].properties,
            Owner::Object(host) => {
                let backing = self.objects[host].backing_class;
                &self.classes[backing].properties
            }
        };
        properties
            .iter()
            .any(|property| self.properties[*property].name == name)
    }

    fn object_has_member_name(&mut self, object: ObjectId, name: &str) -> bool {
        let backing = self.objects[object].backing_class;
        let directly_declared = self.classes[backing]
            .properties
            .iter()
            .any(|property| self.properties[*property].name == name)
            || self.classes[backing]
                .methods
                .iter()
                .any(|method| self.functions[*method].name.rsplit('.').next() == Some(name));
        let ty = self.object_types[self.objects[object].object_type].canonical_type;
        directly_declared
            || self.find_accessible_nominal_property(ty, name).is_some()
            || !self.methods_by_name(ty, name).is_empty()
    }

    fn object_has_property_name(&mut self, object: ObjectId, name: &str) -> bool {
        let backing = self.objects[object].backing_class;
        let directly_declared = self.classes[backing]
            .properties
            .iter()
            .any(|property| self.properties[*property].name == name);
        let ty = self.object_types[self.objects[object].object_type].canonical_type;
        directly_declared || self.find_accessible_nominal_property(ty, name).is_some()
    }

    pub(crate) fn companion_forwarding_object(
        &mut self,
        host: NominalTarget,
        name: &str,
    ) -> Option<ObjectId> {
        if let NominalTarget::Object(object) = host
            && self.object_has_member_name(object, name)
        {
            return None;
        }
        let companion = self.companion_object(host.owner())?;
        self.object_has_member_name(companion, name)
            .then_some(companion)
    }

    pub(crate) fn companion_forwarding_property_object(
        &mut self,
        host: NominalTarget,
        name: &str,
    ) -> Option<ObjectId> {
        if let NominalTarget::Object(object) = host
            && self.object_has_property_name(object, name)
        {
            return None;
        }
        let companion = self.companion_object(host.owner())?;
        self.object_has_property_name(companion, name)
            .then_some(companion)
    }

    pub(crate) fn resolve_object(&mut self, object: ObjectId, source: ObjectSource<'_>) {
        let backing = self.objects[object].backing_class;
        self.type_params_in_scope.clear();
        let mut names = std::collections::HashSet::new();
        let mut fields = Vec::new();
        for (member_index, member) in source.members().iter().enumerate() {
            let ast::ClassMember::StoredProperty(property) = member else {
                if let ast::ClassMember::SecondaryConstructor(constructor) = member {
                    self.error(
                        constructor.span,
                        format!(
                            "object `{}` cannot declare a constructor",
                            self.objects[object].name
                        ),
                    );
                }
                continue;
            };
            if !names.insert(property.name.text.clone()) {
                self.error(
                    property.name.span,
                    format!(
                        "duplicate property `{}` in object `{}`",
                        property.name.text, self.objects[object].name
                    ),
                );
                continue;
            }
            // Const values are evaluated as one dependency graph after
            // top-level constants have been declared. They never receive a
            // field or participate in singleton initialization.
            if matches!(property.body, ast::PropertyBodySyntax::Const(_)) {
                continue;
            }
            let Some(ty) = self.resolve_type_ref(&property.ty) else {
                continue;
            };
            let slot_access = if property.is_override {
                crate::visibility::MemberSlotAccess::Override
            } else if property.modifier != ast::MethodModifier::Final {
                crate::visibility::MemberSlotAccess::Declared
            } else {
                crate::visibility::MemberSlotAccess::None
            };
            let access = self.member_access(
                property.visibility,
                property.name.span,
                "property",
                Owner::Object(object),
                self.current_file,
                slot_access,
            );
            let import_source = self.imports.object_property_source(
                object,
                member_index,
                !self.source_is_current_cone(self.current_file),
            );
            if let Some(field) =
                self.allocate_object_property(object, property, ty, access, import_source)
            {
                fields.push(field);
            }
        }
        self.classes[backing].fields = fields;
        let evaluation_context = self.next_class_constructor_context();
        let constructor = self.class_constructors.alloc(hir::ClassConstructor {
            safety: hir::Safety::Safe,
            no_gc_type_params: Vec::new(),
            owner: backing,
            identity_kind: hir::ClassConstructorIdentityKind::Source,
            access: self.local_declaration_access(),
            parameters: Vec::new(),
            kind: hir::ClassConstructorKind::Primary {
                base: hir::BaseInitialization::Root,
                primary_stores: Vec::new(),
                common_initialization: Vec::new(),
            },
            span: source.span(),
            origin: self.definition_origin(source.span()),
            evaluation_context,
        });
        self.classes[backing].constructors.push(constructor);
        self.class_parameter_calling.insert(constructor, Vec::new());
        self.resolve_class_supertypes(backing, source.supertypes(), source.article_description());
    }
}
