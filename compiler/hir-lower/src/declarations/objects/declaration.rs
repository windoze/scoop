use super::*;

impl Lowerer {
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
        let identity = self.prepare_nominal_identity(NominalIdentityInput {
            name,
            parent: owner,
            access: access.declared,
            type_parameter_count: 0,
            kind: SourceNominalKind::Object,
            file,
            span: source.span(),
        })?;
        let backing_identity = hir::HirNominalIdentity::from_generated_key(
            scoop_identity::GeneratedNominalKey::ObjectBackingClass {
                object: identity
                    .concrete_type_id()
                    .expect("an object declaration has no type parameters"),
            },
        )
        .map_err(|error| {
            let mut diagnostic = Diagnostic::at(
                source.span(),
                format!("cannot derive persistent nominal identity: {error}"),
            );
            diagnostic.file = file;
            self.diagnostics.push(diagnostic);
        })
        .ok()?;
        let backing_class = self.classes.alloc(ClassDecl {
            name: name.to_string(),
            owner: owner.map(Owner::as_nominal_owner),
            access: access.clone(),
            definition: hir::ClassDefinition {
                release_policy: Default::default(),
                modifier: hir::ClassModifier::Final,
                self_application,
                type_params: Vec::new(),
                representation: hir::ClassRepresentation::Declared,
                fields: Vec::new(),
                base_class: None,
                interfaces: Vec::new(),
                interface_implementations: Vec::new(),

                gc_free_pointee_requirements: Vec::new(),
            },
            fields: Vec::new(),
            properties: Vec::new(),
            constructors: Vec::new(),
            methods: Vec::new(),
            span: source.span(),
        });
        let object = self.objects.alloc(hir::ObjectDecl {
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
        self.register_nominal_identity(Owner::Object(object), identity);
        self.register_nominal_identity(Owner::Class(backing_class), backing_identity);
        let canonical_type = self.class_application(backing_class, Vec::new());
        assert_eq!(self.types[canonical_type], Type::Class(self_application));
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
        let published_root = self
            .singleton_published_roots
            .alloc(hir::SingletonPublishedRoot {
                value: singleton_value_id,
                ty: canonical_type,
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
}
