use super::*;

impl Lowerer {
    pub(crate) fn declare_object<'a>(
        &mut self,
        declaration: &'a ast::ObjectDecl,
        pending: &mut Vec<(ObjectId, &'a ast::ObjectDecl, usize)>,
        pending_methods: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file: usize,
        owner: Option<Owner>,
    ) -> Option<ObjectId> {
        self.reject_type_annotations("an object", &declaration.annotations);
        if let Some(kind) = self.type_namespace_conflict(owner, &declaration.name.text) {
            let message = if kind == "an object" {
                format!("duplicate object `{}`", declaration.name.text)
            } else {
                format!(
                    "duplicate type `{}` (already declared as {kind})",
                    declaration.name.text
                )
            };
            self.error(declaration.name.span, message);
            return None;
        }

        let access = match owner {
            Some(owner) => self.nested_nominal_access(
                declaration.visibility,
                declaration.name.span,
                "object",
                owner,
                file,
            ),
            None => self.nominal_access(
                declaration.visibility,
                declaration.name.span,
                "object",
                file,
            ),
        };
        let object_id = ObjectId::from_raw((self.objects.len() as u32).into());
        let object_type_id = hir::ObjectTypeId::from_raw((self.object_types.len() as u32).into());
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
            name: declaration.name.text.clone(),
            owner: owner.map(Owner::as_nominal_owner),
            access: access.clone(),
            self_application,
            type_params: Vec::new(),
            representation: hir::ClassRepresentation::Declared,
            fields: Vec::new(),
            properties: Vec::new(),
            constructors: Vec::new(),
            base_class: None,
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            span: declaration.span,
        });
        let canonical_type = self.class_application(backing_class, Vec::new());
        assert_eq!(self.types[canonical_type], Type::Class(self_application));

        let object = self.objects.alloc(hir::ObjectDecl {
            name: declaration.name.text.clone(),
            owner: owner.map(Owner::as_nominal_owner),
            access: access.clone(),
            object_type: object_type_id,
            singleton_value: singleton_value_id,
            backing_class,
            span: declaration.span,
        });
        assert_eq!(object, object_id);
        let object_type = self.object_types.alloc(hir::ObjectType {
            declaration: object,
            representation: self_application,
            canonical_type,
        });
        assert_eq!(object_type, object_type_id);

        let qualified_name = Owner::Object(object).describe_name(self);
        let stable_key = if access.declared == hir::DeclaredVisibility::Private && owner.is_none() {
            let source = crate::globals::stable_source_identity(&self.intrinsic_sources[file].name);
            format!("object-private:{source}:{qualified_name}")
        } else {
            format!("object:{qualified_name}")
        };
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
            self.allocate_initialization_functions(initialization_id, declaration.span, file);
        let value = self.singleton_values.alloc(hir::SingletonValue {
            declaration: object,
            object_type,
            published_root,
            initialization: initialization_id,
        });
        assert_eq!(value, singleton_value_id);
        let initialization = self.initialization_units.alloc(hir::InitializationUnit {
            stable_key,
            schedule: hir::InitializationSchedule::LazyAccess,
            kind: hir::InitializationUnitKind::LazySingleton {
                value,
                published_root,
            },
            initializer,
            ensure,
            failure_root,
            dependencies: Vec::new(),
            span: declaration.span,
        });
        assert_eq!(initialization, initialization_id);

        match owner {
            Some(owner) => {
                self.nested_nominals_by_owner.insert(
                    (owner, declaration.name.text.clone()),
                    NominalTarget::Object(object),
                );
            }
            None => {
                self.objects_by_name
                    .insert(declaration.name.text.clone(), object);
            }
        }
        self.object_by_backing_class.insert(backing_class, object);
        self.class_files.insert(backing_class, file);
        self.object_files.insert(object, file);
        for method in declaration
            .members
            .iter()
            .filter_map(|member| match member {
                ast::ClassMember::Function(function) => Some(function),
                _ => None,
            })
        {
            self.declare_method(method, Owner::Object(object), pending_methods, file);
        }
        pending.push((object, declaration, file));
        Some(object)
    }

    pub(crate) fn resolve_object(&mut self, object: ObjectId, declaration: &ast::ObjectDecl) {
        let backing = self.objects[object].backing_class;
        self.type_params_in_scope.clear();
        let mut names = std::collections::HashSet::new();
        let mut fields = Vec::new();
        for member in &declaration.members {
            let ast::ClassMember::StoredProperty(property) = member else {
                if let ast::ClassMember::SecondaryConstructor(constructor) = member {
                    self.error(
                        constructor.span,
                        format!(
                            "object `{}` cannot declare a constructor",
                            declaration.name.text
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
                        property.name.text, declaration.name.text
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
            if let Some(field) = self.allocate_object_property(object, property, ty, access) {
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
            span: declaration.span,
            origin: self.definition_origin(declaration.span),
        });
        self.classes[backing].constructors.push(constructor);
        self.class_parameter_calling.insert(constructor, Vec::new());
        self.resolve_class_supertypes(backing, &declaration.supertypes, "an object");
    }
}
