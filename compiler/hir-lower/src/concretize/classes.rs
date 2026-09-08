use super::*;

impl Concretizer<'_> {
    pub(super) fn ensure_class(
        &mut self,
        source_id: export::ClassId,
        arguments: Vec<concrete::TypeId>,
        application: ConcreteApplicationRepresentation,
    ) -> concrete::ClassId {
        let key = (source_id, arguments.clone());
        if let Some(&id) = self.class_by_key.get(&key) {
            return id;
        }
        let source = self.source.classes[source_id].clone();
        assert_eq!(source.type_params.len(), arguments.len());
        let representation = match (&source.representation, application) {
            (
                export::ClassRepresentation::Declared,
                ConcreteApplicationRepresentation::Declared,
            ) => concrete::ClassRepresentation::Declared {
                fields: Vec::new(),
                base_class: None,
            },
            (
                export::ClassRepresentation::Intrinsic(declaration),
                ConcreteApplicationRepresentation::Intrinsic(application),
            ) => concrete::ClassRepresentation::Intrinsic {
                declaration: *declaration,
                application,
            },
            _ => unreachable!("ExportHir declaration and application representations agree"),
        };
        let id = self.classes.alloc(concrete::ClassDef {
            origin: concrete::ClassOriginId::from_raw(source_id.into_raw().into_u32()),
            modifier: source.modifier,
            link_stem: source.link_stem.clone(),
            name: self.instance_name(
                &self.source_nominal_name(&source.name, source.owner),
                &arguments,
            ),
            owner: Self::lower_nominal_owner(source.owner),
            type_arguments: arguments.clone(),
            representation,
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            span: source.span,
        });
        self.class_by_key.insert(key, id);
        self.class_source.insert(id, source_id);
        let ty = match self.classes[id].representation {
            concrete::ClassRepresentation::Intrinsic {
                application: concrete::IntrinsicTypeRepresentation::String,
                ..
            } => self.intern_type(concrete::TypeKind::String, false),
            concrete::ClassRepresentation::Declared { .. }
            | concrete::ClassRepresentation::Intrinsic {
                application:
                    concrete::IntrinsicTypeRepresentation::Array { .. }
                    | concrete::IntrinsicTypeRepresentation::MutableArray { .. },
                ..
            } => self.intern_type(concrete::TypeKind::Class(id), false),
            concrete::ClassRepresentation::Intrinsic { .. } => {
                unreachable!("the registry fixes intrinsic declaration targets")
            }
        };
        self.class_type.insert(id, ty);

        // Reserve every initializer before lowering any body. `this` cycles
        // have already been rejected in Export HIR, while recursive type
        // references can still encounter these identities during lowering.
        if matches!(source.representation, export::ClassRepresentation::Declared) {
            for &constructor in &source.constructors {
                let raw = self.class_constructor_slots.len() as u32;
                self.class_constructor_slots.push(None);
                let concrete = concrete::ClassConstructorId::from_raw(raw.into());
                assert!(
                    self.class_constructor_by_key
                        .insert((constructor, id), concrete)
                        .is_none()
                );
            }
        }

        let fields: Vec<concrete::Field> = source
            .fields
            .iter()
            .map(|field| &self.source.class_fields[*field])
            .map(|field| concrete::Field {
                name: self.source.properties[field.property].name.clone(),
                ty: self.lower_type(field.ty, &arguments),
            })
            .collect();
        let method_owner = self.object_by_backing_class.get(&source_id).map_or(
            concrete::MethodOwner::Class(id),
            |object| {
                concrete::MethodOwner::Object(concrete::ObjectTypeId::from_raw(
                    self.source.objects[*object].object_type.into_raw(),
                ))
            },
        );
        let methods = self.request_concrete_methods(&source.methods, method_owner);
        let interface_implementations =
            self.lower_interface_implementations(&source.interface_implementations, &arguments);
        let interfaces = interface_implementations
            .iter()
            .map(|implementation| self.interface_type[&implementation.interface])
            .collect();
        let base_class = source.base_class.map(|base| {
            let base = self.lower_type(base, &arguments);
            let concrete::TypeKind::Class(base) = self.types[base].kind else {
                unreachable!("class bases concretize to class identities")
            };
            base
        });
        self.classes[id].interfaces = interfaces;
        self.classes[id].interface_implementations = interface_implementations;
        match &mut self.classes[id].representation {
            concrete::ClassRepresentation::Declared {
                fields: concrete_fields,
                base_class: concrete_base,
            } => {
                *concrete_fields = fields;
                *concrete_base = base_class;
            }
            concrete::ClassRepresentation::Intrinsic { .. } => {
                debug_assert!(fields.is_empty() && base_class.is_none());
            }
        }
        self.classes[id].methods = methods;
        for &constructor in &source.constructors {
            let concrete = self.lower_class_constructor(constructor, id, &arguments);
            let target = self.class_constructor_by_key[&(constructor, id)];
            let slot = target.into_raw().into_u32() as usize;
            assert!(
                self.class_constructor_slots[slot]
                    .replace(concrete)
                    .is_none()
            );
        }
        id
    }

    fn lower_class_constructor(
        &mut self,
        source_id: export::ClassConstructorId,
        class: concrete::ClassId,
        substitution: &[concrete::TypeId],
    ) -> concrete::ClassConstructor {
        let source = self.source.class_constructors[source_id].clone();
        let parameters = source
            .parameters
            .iter()
            .map(|parameter| concrete::ConstructorParameter {
                id: concrete::ConstructorParamId::from_raw(parameter.id.into_raw()),
                name: parameter.name.clone(),
                ty: self.lower_type(parameter.ty, substitution),
            })
            .collect::<Vec<_>>();
        let mut body = concrete::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        };
        let class_ty = self.class_type[&class];
        let kind = match &source.kind {
            export::ClassConstructorKind::Primary {
                base,
                primary_stores,
                common_initialization,
            } => {
                self.append_base_initialization(
                    &mut body,
                    base,
                    substitution,
                    source.span,
                    source.origin,
                );
                for store in primary_stores {
                    let field = self.source.class_fields[store.field].clone();
                    let application = self.source.classes[field.owner].self_application;
                    let receiver_ty = self.lower_type(
                        self.source.class_applications[application].canonical_type,
                        substitution,
                    );
                    let receiver =
                        self.constructor_receiver(receiver_ty, store.span, source.origin);
                    let value_ty = self.lower_type(field.ty, substitution);
                    let value = concrete::Expr {
                        kind: concrete::ExprKind::ConstructorParam(
                            concrete::ConstructorParamId::from_raw(store.parameter.into_raw()),
                        ),
                        ty: value_ty,
                        span: store.span,
                        origin: export::ExpressionOrigin::Definition(source.origin).concrete(),
                    };
                    body.statements.push(concrete::Statement {
                        kind: concrete::StatementKind::Assign {
                            target: concrete::AssignTarget::Field {
                                receiver: Box::new(receiver),
                                field: concrete::FieldRef::ClassField {
                                    class_id: self
                                        .lower_class_application(application, substitution),
                                    index: self.source_class_field_layout_index(store.field),
                                },
                            },
                            value,
                        },
                        span: store.span,
                    });
                }
                self.append_common_initialization(
                    &mut body,
                    common_initialization,
                    substitution,
                    source.origin,
                );
                concrete::ClassConstructorKind::Terminal { body }
            }
            export::ClassConstructorKind::Secondary {
                delegation,
                body: secondary_body,
            } => match delegation {
                export::ClassSecondaryDelegation::This { target, arguments } => {
                    let target_id = self.lower_class_constructor_application(*target, substitution);
                    let args =
                        self.append_constructor_arguments(&mut body, arguments, substitution);
                    let receiver = self.constructor_receiver(class_ty, source.span, source.origin);
                    body.statements.push(concrete::Statement {
                        kind: concrete::StatementKind::Expr(concrete::Expr {
                            kind: concrete::ExprKind::ClassInitializerCall {
                                receiver: Box::new(receiver),
                                initializer: target_id,
                                args,
                            },
                            ty: self.lower_type(self.source.unit, &[]),
                            span: source.span,
                            origin: export::ExpressionOrigin::Definition(source.origin).concrete(),
                        }),
                        span: source.span,
                    });
                    self.append_source_body(&mut body, secondary_body, substitution);
                    concrete::ClassConstructorKind::This {
                        target: target_id,
                        body,
                    }
                }
                export::ClassSecondaryDelegation::Terminal {
                    base,
                    common_initialization,
                } => {
                    self.append_base_initialization(
                        &mut body,
                        base,
                        substitution,
                        source.span,
                        source.origin,
                    );
                    self.append_common_initialization(
                        &mut body,
                        common_initialization,
                        substitution,
                        source.origin,
                    );
                    self.append_source_body(&mut body, secondary_body, substitution);
                    concrete::ClassConstructorKind::Terminal { body }
                }
            },
        };
        concrete::ClassConstructor {
            class,
            source_discriminator: source_id.into_raw().into_u32(),
            parameters,
            kind,
        }
    }

    pub(super) fn lower_class_constructor_application(
        &mut self,
        source: export::ClassConstructorApplicationId,
        substitution: &[concrete::TypeId],
    ) -> concrete::ClassConstructorId {
        let application = &self.source.class_constructor_applications[source];
        let class = self.lower_class_application(application.owner, substitution);
        self.class_constructor_by_key[&(application.constructor, class)]
    }

    fn append_base_initialization(
        &mut self,
        body: &mut concrete::Body,
        base: &export::BaseInitialization,
        substitution: &[concrete::TypeId],
        span: scoop_ast::Span,
        origin: export::DefinitionOrigin,
    ) {
        let export::BaseInitialization::Super { target, arguments } = base else {
            return;
        };
        let target = self.lower_class_constructor_application(*target, substitution);
        let args = self.append_constructor_arguments(body, arguments, substitution);
        let target_class = self.class_constructors_slot(target).class;
        let target_ty = self.class_type[&target_class];
        let receiver = self.constructor_receiver(target_ty, span, origin);
        body.statements.push(concrete::Statement {
            kind: concrete::StatementKind::Expr(concrete::Expr {
                kind: concrete::ExprKind::ClassInitializerCall {
                    receiver: Box::new(receiver),
                    initializer: target,
                    args,
                },
                ty: self.lower_type(self.source.unit, &[]),
                span,
                origin: export::ExpressionOrigin::Definition(origin).concrete(),
            }),
            span,
        });
    }

    fn class_constructors_slot(
        &self,
        constructor: concrete::ClassConstructorId,
    ) -> &concrete::ClassConstructor {
        self.class_constructor_slots[constructor.into_raw().into_u32() as usize]
            .as_ref()
            .expect("base class initializers are concretized before derived initializers")
    }

    fn append_common_initialization(
        &mut self,
        body: &mut concrete::Body,
        common: &[export::ClassInitializationStep],
        substitution: &[concrete::TypeId],
        origin: export::DefinitionOrigin,
    ) {
        for step in common {
            match step {
                export::ClassInitializationStep::StoredProperty {
                    field,
                    initializer,
                    span,
                }
                | export::ClassInitializationStep::DelegatedProperty {
                    field,
                    initializer,
                    span,
                    ..
                } => {
                    let locals = self.append_source_locals(body, &initializer.locals, substitution);
                    body.statements.extend(self.lower_statement_region(
                        &initializer.statements,
                        substitution,
                        &locals,
                    ));
                    let value = self.lower_expr(&initializer.value, substitution, &locals);
                    let source_field = &self.source.class_fields[*field];
                    let application = self.source.classes[source_field.owner].self_application;
                    let class = self.lower_class_application(application, substitution);
                    let receiver_ty = self.class_type[&class];
                    let receiver = self.constructor_receiver(receiver_ty, *span, origin);
                    body.statements.push(concrete::Statement {
                        kind: concrete::StatementKind::Assign {
                            target: concrete::AssignTarget::Field {
                                receiver: Box::new(receiver),
                                field: concrete::FieldRef::ClassField {
                                    class_id: class,
                                    index: self.source_class_field_layout_index(*field),
                                },
                            },
                            value,
                        },
                        span: *span,
                    });
                }
                export::ClassInitializationStep::InitBlock {
                    body: source_body, ..
                } => self.append_source_body(body, source_body, substitution),
            }
        }
    }

    pub(super) fn append_constructor_arguments(
        &mut self,
        body: &mut concrete::Body,
        arguments: &export::ConstructorArguments,
        substitution: &[concrete::TypeId],
    ) -> Vec<concrete::Expr> {
        let locals = self.append_source_locals(body, &arguments.locals, substitution);
        body.statements.extend(self.lower_statement_region(
            &arguments.statements,
            substitution,
            &locals,
        ));
        arguments
            .args
            .iter()
            .map(|argument| self.lower_expr(argument, substitution, &locals))
            .collect()
    }

    pub(super) fn append_source_body(
        &mut self,
        body: &mut concrete::Body,
        source: &export::Body,
        substitution: &[concrete::TypeId],
    ) {
        let locals = self.append_source_locals(body, &source.locals, substitution);
        body.statements.extend(self.lower_statement_region(
            &source.statements,
            substitution,
            &locals,
        ));
    }

    pub(super) fn append_source_locals(
        &mut self,
        body: &mut concrete::Body,
        source: &Arena<export::Local>,
        substitution: &[concrete::TypeId],
    ) -> Vec<concrete::LocalId> {
        source
            .iter()
            .map(|(_, local)| {
                let ty = self.lower_type(local.ty, substitution);
                body.locals.alloc(concrete::Local {
                    binding: concrete::BindingId::from_raw(local.binding.into_raw()),
                    name: local.name.clone(),
                    ty,
                    mutable: local.mutable,
                })
            })
            .collect()
    }

    fn constructor_receiver(
        &self,
        ty: concrete::TypeId,
        span: scoop_ast::Span,
        origin: export::DefinitionOrigin,
    ) -> concrete::Expr {
        concrete::Expr {
            kind: concrete::ExprKind::ConstructorReceiver,
            ty,
            span,
            origin: export::ExpressionOrigin::Definition(origin).concrete(),
        }
    }

    pub(super) fn request_concrete_methods(
        &mut self,
        source_methods: &[export::FunctionId],
        owner: concrete::MethodOwner,
    ) -> Vec<concrete::FunctionId> {
        source_methods
            .iter()
            .copied()
            .filter_map(|method| {
                let function = &self.source.functions[method];
                match function.genericity {
                    export::FunctionGenericity::Plain
                    | export::FunctionGenericity::OwnerParameterizedMethod { .. } => {
                        Some(self.request_method(method, owner, MethodRequest::Plain))
                    }
                    export::FunctionGenericity::GenericMethod { .. } => None,
                    export::FunctionGenericity::Generic { .. } => {
                        unreachable!("nominal method lists do not contain generic functions")
                    }
                }
            })
            .collect()
    }

    pub(super) fn lower_interface_implementations(
        &mut self,
        source_implementations: &[export::InterfaceImplementation],
        substitution: &[concrete::TypeId],
    ) -> Vec<concrete::InterfaceImplementation> {
        source_implementations
            .iter()
            .cloned()
            .map(|implementation| {
                let interface =
                    self.lower_interface_application(implementation.interface, substitution);
                let methods = implementation
                    .methods
                    .into_iter()
                    .map(|method| {
                        let slot = self.interface_slot_by_source[&(interface, method.member)];
                        let target = match method.target {
                            export::InterfaceImplementationTarget::Method(application) => {
                                let concrete::Callable::Function(function) =
                                    self.lower_method_application(application, substitution);
                                concrete::InterfaceImplementationTarget::Method(function)
                            }
                            export::InterfaceImplementationTarget::Subclass => {
                                let source = self.source.interface_methods[method.member].function;
                                let declaration = self.request_method(
                                    source,
                                    concrete::MethodOwner::Interface(interface),
                                    MethodRequest::Plain,
                                );
                                concrete::InterfaceImplementationTarget::Abstract { declaration }
                            }
                        };
                        concrete::InterfaceMethodImplementation { slot, target }
                    })
                    .collect();
                concrete::InterfaceImplementation { interface, methods }
            })
            .collect()
    }

    pub(super) fn lower_extern_functions(&mut self) {
        for (source_id, source) in self.source.extern_functions.iter() {
            let params = source
                .params
                .iter()
                .map(|ty| self.lower_type(*ty, &[]))
                .collect();
            let return_type = self.lower_type(source.return_type, &[]);
            let id = self.extern_functions.alloc(concrete::ExternFunction {
                source_name: source.source_name.clone(),
                native_symbol: source.native_symbol.clone(),
                library: source.library.clone(),
                abi: source.abi,
                calling_convention: source.calling_convention,
                gc_effect: source.gc_effect,
                safety: source.safety,
                params,
                return_type,
            });
            self.extern_map.insert(source_id, id);
        }
    }

    pub(super) fn lower_globals(&mut self) {
        // Allocate ids first because expressions in function bodies may refer
        // to any global regardless of declaration order.
        for (source_id, source) in self.source.globals.iter() {
            let ty = self.lower_type(source.ty, &[]);
            let storage = self.lower_global_storage(&source.storage);
            let id = self.globals.alloc(concrete::Global {
                name: source.name.clone(),
                ty,
                mutable: source.mutable,
                storage,
                span: source.span,
            });
            self.global_map.insert(source_id, id);
        }
    }

    pub(super) fn lower_global_storage(
        &mut self,
        storage: &export::GlobalStorage,
    ) -> concrete::GlobalStorage {
        match storage {
            export::GlobalStorage::Managed { state } => concrete::GlobalStorage::Managed {
                state: match state {
                    export::HirStaticInitialState::EncodedStaticValue { payload } => {
                        concrete::HirStaticInitialState::EncodedStaticValue {
                            payload: self.lower_constant(payload),
                        }
                    }
                    export::HirStaticInitialState::ZeroedForRuntimeUnit { unit } => {
                        concrete::HirStaticInitialState::ZeroedForRuntimeUnit {
                            unit: concrete::InitializationUnitId::from_raw(unit.into_raw()),
                        }
                    }
                },
            },
            export::GlobalStorage::Local {
                thread_local,
                initializer,
            } => concrete::GlobalStorage::Local {
                thread_local: *thread_local,
                initializer: self.lower_constant(initializer),
            },
            export::GlobalStorage::Extern {
                library,
                native_symbol,
                thread_local,
            } => concrete::GlobalStorage::Extern {
                library: library.clone(),
                native_symbol: native_symbol.clone(),
                thread_local: *thread_local,
            },
        }
    }

    pub(super) fn lower_constant(
        &mut self,
        value: &export::HirConstantImage,
    ) -> concrete::HirConstantImage {
        match value {
            export::HirConstantImage::Integer(value) => concrete::HirConstantImage::Integer(*value),
            export::HirConstantImage::Boolean(value) => concrete::HirConstantImage::Boolean(*value),
            export::HirConstantImage::String(value) => {
                concrete::HirConstantImage::String(value.clone())
            }
            export::HirConstantImage::NullPointer(kind) => {
                concrete::HirConstantImage::NullPointer(match kind {
                    export::HirPointerNullKind::Raw => concrete::HirPointerNullKind::Raw,
                    export::HirPointerNullKind::Code => concrete::HirPointerNullKind::Code,
                })
            }
            export::HirConstantImage::EnumUnit { variant } => {
                concrete::HirConstantImage::EnumUnit {
                    variant: self.lower_applied_enum_variant_ref(*variant, &[]),
                }
            }
            export::HirConstantImage::Struct {
                application,
                fields,
            } => {
                let struct_id = self.lower_struct_application(*application, &[]);
                concrete::HirConstantImage::Struct {
                    struct_id,
                    fields: fields
                        .iter()
                        .map(|field| self.lower_constant(field))
                        .collect(),
                }
            }
        }
    }
}
