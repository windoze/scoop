use super::*;

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

    pub(crate) fn declare_object<'a>(
        &mut self,
        declaration: &'a ast::ObjectDecl,
        pending: &mut Vec<(ObjectId, ObjectSource<'a>, usize)>,
        pending_methods: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file: usize,
        owner: Option<Owner>,
    ) -> Option<ObjectId> {
        self.declare_singleton(
            ObjectSource::Object(declaration),
            pending,
            pending_methods,
            file,
            owner,
        )
    }

    pub(crate) fn declare_companion<'a>(
        &mut self,
        declaration: &'a ast::CompanionObjectDecl,
        pending: &mut Vec<(ObjectId, ObjectSource<'a>, usize)>,
        pending_methods: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file: usize,
        owner: Owner,
    ) -> Option<ObjectId> {
        self.declare_singleton(
            ObjectSource::Companion(declaration),
            pending,
            pending_methods,
            file,
            Some(owner),
        )
    }

    fn declare_singleton<'a>(
        &mut self,
        source: ObjectSource<'a>,
        pending: &mut Vec<(ObjectId, ObjectSource<'a>, usize)>,
        pending_methods: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file: usize,
        owner: Option<Owner>,
    ) -> Option<ObjectId> {
        let (name, name_span) = source.name();
        self.reject_type_annotations(source.article_description(), source.annotations());
        if matches!(source, ObjectSource::Companion(_))
            && owner.is_some_and(|host| self.companion_by_host.contains_key(&host))
        {
            self.error(
                source.span(),
                "a nominal declaration may contain at most one companion object".to_string(),
            );
            return None;
        }
        let lookup_names = if matches!(source, ObjectSource::Companion(_)) && name != "Companion" {
            vec![name, "Companion"]
        } else {
            vec![name]
        };
        if let Some((conflict, kind)) = lookup_names.iter().find_map(|lookup_name| {
            self.type_namespace_conflict(
                owner,
                lookup_name,
                file,
                crate::namespace::is_file_private(source.visibility()),
            )
            .map(|kind| (*lookup_name, kind))
        }) {
            let message = if matches!(source, ObjectSource::Companion(_)) {
                format!(
                    "companion object name `{conflict}` conflicts with {kind} in the same owner"
                )
            } else if kind == "an object" {
                format!("duplicate object `{name}`")
            } else {
                format!("duplicate type `{name}` (already declared as {kind})")
            };
            self.error(name_span, message);
            return None;
        }

        let access = match owner {
            Some(owner) => self.nested_nominal_access(
                source.visibility(),
                name_span,
                source.description(),
                owner,
                file,
            ),
            None => self.nominal_access(source.visibility(), name_span, source.description(), file),
        };
        let private = access.declared == hir::DeclaredVisibility::Private;
        let object_link_stem = self.local_nominal_link_stem(
            file,
            owner,
            name,
            private,
            crate::globals::LocalNominalLinkRole::Object,
        );
        let backing_link_stem = self.local_nominal_link_stem(
            file,
            owner,
            name,
            private,
            crate::globals::LocalNominalLinkRole::ObjectBackingClass,
        );
        let object_id = ObjectId::from_raw((self.objects.len() as u32).into());
        let object_type_id = hir::ObjectTypeId::from_raw((self.object_types.len() as u32).into());
        let companion_relation_id = source.companion_name().map(|_| {
            hir::CompanionRelationId::from_raw((self.companion_relations.len() as u32).into())
        });
        let singleton_value_id =
            hir::SingletonValueId::from_raw((self.singleton_values.len() as u32).into());
        let published_root_id = hir::SingletonPublishedRootId::from_raw(
            (self.singleton_published_roots.len() as u32).into(),
        );
        let initialization_id =
            hir::InitializationUnitId::from_raw((self.initialization_units.len() as u32).into());

        let self_application =
            hir::ClassApplicationId::from_raw((self.class_applications.len() as u32).into());
        let backing_class = self.classes.alloc(ClassDecl {
            modifier: hir::ClassModifier::Final,
            link_stem: backing_link_stem,
            name: name.to_string(),
            owner: owner.map(Owner::as_nominal_owner),
            access: access.clone(),
            self_application,
            type_params: Vec::new(),
            gc_free_pointee_requirements: Vec::new(),
            representation: hir::ClassRepresentation::Declared,
            fields: Vec::new(),
            properties: Vec::new(),
            constructors: Vec::new(),
            base_class: None,
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            span: source.span(),
        });
        let canonical_type = self.class_application(backing_class, Vec::new());
        assert_eq!(self.types[canonical_type], Type::Class(self_application));

        let object = self.objects.alloc(hir::ObjectDecl {
            link_stem: object_link_stem,
            name: name.to_string(),
            owner: owner.map(Owner::as_nominal_owner),
            access: access.clone(),
            object_type: object_type_id,
            singleton_value: singleton_value_id,
            kind: companion_relation_id
                .map_or(hir::ObjectKind::Standalone, hir::ObjectKind::Companion),
            backing_class,
            span: source.span(),
        });
        assert_eq!(object, object_id);
        let object_type = self.object_types.alloc(hir::ObjectType {
            declaration: object,
            representation: self_application,
            canonical_type,
        });
        assert_eq!(object_type, object_type_id);

        if let (Some(relation), Some(host), Some(companion_name)) =
            (companion_relation_id, owner, source.companion_name())
        {
            let allocated = self.companion_relations.alloc(hir::CompanionRelation {
                host: host.as_nominal_owner(),
                object,
                name: companion_name,
            });
            assert_eq!(allocated, relation);
            assert!(self.companion_by_host.insert(host, relation).is_none());
        }

        self.object_files.insert(object, file);
        let display_name = self.singleton_initialization_display_name(object);
        let stable_key = self.local_link_component(
            file,
            Some(Owner::Object(object)),
            "",
            false,
            crate::globals::LocalLinkRole::SingletonInitialization,
        );
        let published_root = self
            .singleton_published_roots
            .alloc(hir::SingletonPublishedRoot {
                value: singleton_value_id,
                ty: canonical_type,
                link_name: stable_key.clone(),
            });
        assert_eq!(published_root, published_root_id);
        let failure_root =
            self.initialization_failure_roots
                .alloc(hir::InitializationFailureRoot {
                    unit: initialization_id,
                });
        let (initializer, ensure) =
            self.allocate_initialization_functions(initialization_id, source.span(), file);
        let value = self.singleton_values.alloc(hir::SingletonValue {
            declaration: object,
            object_type,
            published_root,
            initialization: initialization_id,
        });
        assert_eq!(value, singleton_value_id);
        let initialization = self.initialization_units.alloc(hir::InitializationUnit {
            stable_key,
            display_name,
            schedule: hir::InitializationSchedule::LazyAccess,
            kind: hir::InitializationUnitKind::LazySingleton {
                value,
                published_root,
            },
            initializer,
            ensure,
            failure_root,
            dependencies: Vec::new(),
            span: source.span(),
        });
        assert_eq!(initialization, initialization_id);

        match owner {
            Some(owner) => {
                for lookup_name in lookup_names {
                    self.nested_nominals_by_owner.insert(
                        (owner, lookup_name.to_string()),
                        NominalTarget::Object(object),
                    );
                }
            }
            None => {
                self.top_level_namespaces.register_type(
                    file,
                    name.to_string(),
                    crate::namespace::TopLevelTypeTarget::Nominal(NominalTarget::Object(object)),
                    self.objects[object].access.declared == hir::DeclaredVisibility::Private,
                );
            }
        }
        self.object_by_backing_class.insert(backing_class, object);
        self.class_files.insert(backing_class, file);
        for method in source.members().iter().filter_map(|member| match member {
            ast::ClassMember::Function(function) => Some(function),
            _ => None,
        }) {
            self.declare_method(method, Owner::Object(object), pending_methods, file);
        }
        pending.push((object, source, file));
        Some(object)
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
                self.current_source_is_core(),
            );
            if let Some(field) =
                self.allocate_object_property(object, property, ty, access, import_source)
            {
                fields.push(field);
            }
        }
        self.classes[backing].fields = fields;
        let constructor = self.class_constructors.alloc(hir::ClassConstructor {
            owner: backing,
            access: self.local_declaration_access(),
            parameters: Vec::new(),
            kind: hir::ClassConstructorKind::Primary {
                base: hir::BaseInitialization::Root,
                primary_stores: Vec::new(),
                common_initialization: Vec::new(),
            },
            span: source.span(),
            origin: self.definition_origin(source.span()),
        });
        self.classes[backing].constructors.push(constructor);
        self.class_parameter_calling.insert(constructor, Vec::new());
        self.resolve_class_supertypes(backing, source.supertypes(), source.article_description());
    }
}
