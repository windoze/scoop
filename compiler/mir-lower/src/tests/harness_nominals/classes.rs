use super::super::*;

impl Harness {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::tests) fn class(
        &mut self,
        name: &str,
        modifier: hir::ClassModifier,
        constructor: &[(&str, hir::TypeId)],
        base: Option<(hir::ClassId, Vec<hir::Expr>)>,
        interfaces: &[hir::InterfaceId],
    ) -> hir::ClassId {
        let interfaces: Vec<_> = interfaces
            .iter()
            .map(|&interface| self.interface_ty(interface))
            .collect();
        let base = base.map(|(base, arguments)| (self.class_ty(base), base, arguments));
        self.declare_class(name, modifier, constructor, base, interfaces)
    }

    pub(in crate::tests) fn declare_class(
        &mut self,
        name: &str,
        modifier: hir::ClassModifier,
        constructor: &[(&str, hir::TypeId)],
        base_class: Option<(hir::TypeId, hir::ClassId, Vec<hir::Expr>)>,
        interfaces: Vec<hir::TypeId>,
    ) -> hir::ClassId {
        let mut interface_implementations = base_class
            .as_ref()
            .map(|(base, _, _)| {
                let hir::Type::Class(application) = self.types[*base] else {
                    panic!("test harness class bases are class applications")
                };
                let base = self.class_applications[application].template;
                self.classes[base].interface_implementations.clone()
            })
            .unwrap_or_default();
        for implementation in self.interface_implementation_shells(&interfaces) {
            if let Some(existing) = interface_implementations
                .iter_mut()
                .find(|existing| existing.interface == implementation.interface)
            {
                *existing = implementation;
            } else {
                interface_implementations.push(implementation);
            }
        }
        let self_application =
            hir::ClassApplicationId::from_raw((self.class_applications.len() as u32).into());
        let class = hir::ClassId::from_raw((self.classes.len() as u32).into());
        let parameters = constructor
            .iter()
            .map(|(name, ty)| {
                let id = hir::ConstructorParamId::from_raw(self.next_constructor_param);
                self.next_constructor_param += 1;
                hir::ConstructorParameter {
                    id,
                    name: name.to_string(),
                    ty: *ty,
                }
            })
            .collect::<Vec<_>>();
        let mut fields = Vec::with_capacity(parameters.len());
        let mut properties = Vec::with_capacity(parameters.len());
        for parameter in &parameters {
            let property = hir::PropertyId::from_raw((self.properties.len() as u32).into());
            let getter = self.property_getters.alloc(hir::PropertyGetter {
                access: hir::DeclarationAccess::public(),
                implementation: hir::PropertyAccessorImplementation::Storage,
                attributes: hir::FunctionAttributes::default(),
                span: SPAN,
            });
            let field = self.class_fields.alloc(hir::ClassField {
                owner: class,
                property,
                ty: parameter.ty,
                source: hir::ClassFieldSource::PrimaryParameter(parameter.id),
                span: SPAN,
            });
            let actual = self.properties.alloc(hir::Property {
                owner: hir::PropertyOwner::Class(class),
                name: parameter.name.clone(),
                access: hir::DeclarationAccess::public(),
                modifier: hir::MethodModifier::Final,
                is_override: false,
                overrides: Vec::new(),
                override_access: Vec::new(),
                ty: parameter.ty,
                capability: hir::PropertyCapability::ReadOnly { getter },
                representation: hir::PropertyRepresentation::Stored(hir::StoredProperty {
                    backing: hir::PropertyBacking::ClassField {
                        field,
                        initializer: hir::ClassPropertyInitializer::PrimaryParameter(parameter.id),
                    },
                }),
                span: SPAN,
            });
            assert_eq!(actual, property);
            fields.push(field);
            properties.push(property);
        }
        let base_initialization = match &base_class {
            None => hir::BaseInitialization::Root,
            Some((_, base, arguments)) => {
                let target = self.classes[*base].constructors[0];
                let owner = self.classes[*base].self_application;
                let target =
                    self.class_constructor_applications
                        .alloc(hir::ClassConstructorApplication {
                            constructor: target,
                            owner,
                        });
                hir::BaseInitialization::Super {
                    target,
                    arguments: hir::ConstructorArguments {
                        locals: Arena::new(),
                        statements: Vec::new(),
                        args: arguments.clone(),
                    },
                }
            }
        };
        let primary_stores = fields
            .iter()
            .copied()
            .zip(parameters.iter())
            .map(|(field, parameter)| hir::PrimaryFieldStore {
                field,
                parameter: parameter.id,
                span: SPAN,
            })
            .collect();
        let constructor_id = self.class_constructors.alloc(hir::ClassConstructor {
            owner: class,
            access: hir::DeclarationAccess::public(),
            parameters,
            kind: hir::ClassConstructorKind::Primary {
                base: base_initialization,
                primary_stores,
                common_initialization: Vec::new(),
            },
            span: SPAN,
            origin: definition_origin(),
        });
        let class = self.classes.alloc(hir::ClassDecl {
            owner: None,
            modifier,
            link_stem: nominal_link_stem(name),
            name: name.to_string(),
            access: hir::NominalAccess::public(),
            self_application,
            type_params: Vec::new(),
            gc_free_pointee_requirements: Vec::new(),
            representation: hir::ClassRepresentation::Declared,
            fields,
            properties,
            constructors: vec![constructor_id],
            base_class: base_class.as_ref().map(|(ty, _, _)| *ty),
            interfaces,
            interface_implementations,
            methods: Vec::new(),
            span: SPAN,
        });
        let actual = self.class_application(class, Vec::new());
        assert_eq!(actual, self_application);
        class
    }

    /// A concrete zero-argument exception shell used by tests that exercise
    /// compiler-generated exception edges.
    pub(in crate::tests) fn exception(&mut self, name: &str) -> hir::ClassId {
        self.class(name, hir::ClassModifier::Final, &[], None, &[])
    }

    pub(in crate::tests) fn exception_target(
        &mut self,
        name: &str,
        include: bool,
    ) -> hir::CompilerException {
        let existing = self
            .classes
            .iter()
            .find_map(|(id, declaration)| (declaration.name == name).then_some(id));
        let class = if let Some(existing) = existing {
            existing
        } else if include {
            self.exception(name)
        } else {
            self.class(
                &format!("${name}Protocol"),
                // LocalConcreteHir's exception contract always includes a
                // real constructor callable. The protocol shell remains
                // hidden from unrelated dump assertions by the test helper.
                hir::ClassModifier::Final,
                &[],
                None,
                &[],
            )
        };
        hir::CompilerException {
            constructor: hir::ZeroArgClassConstructor {
                class,
                constructor: self.classes[class].constructors[0],
            },
        }
    }

    pub(in crate::tests) fn test_exception_core(
        &mut self,
        include: bool,
    ) -> hir::CompilerExceptionCore {
        let illegal_state_exception = self.exception_target("IllegalStateException", include);
        let illegal_state_class = illegal_state_exception.constructor.class;
        let zero_argument_constructor = illegal_state_exception.constructor.constructor;
        // Focused lowering fixtures without initialization units never consume
        // this capability. Keep their nominal graphs minimal; fixtures that
        // exercise initialization use the exact core Option<String> contract.
        let message_type = if self.needs_initialization_core {
            self.option(self.string)
        } else {
            self.string
        };
        let parameter = hir::ConstructorParameter {
            id: hir::ConstructorParamId::from_raw(self.next_constructor_param),
            name: "message".to_string(),
            ty: message_type,
        };
        self.next_constructor_param += 1;
        let owner = self.classes[illegal_state_class].self_application;
        let target = self
            .class_constructor_applications
            .alloc(hir::ClassConstructorApplication {
                constructor: zero_argument_constructor,
                owner,
            });
        let message_constructor = self.class_constructors.alloc(hir::ClassConstructor {
            owner: illegal_state_class,
            access: hir::DeclarationAccess::public(),
            parameters: vec![parameter],
            kind: hir::ClassConstructorKind::Secondary {
                delegation: hir::ClassSecondaryDelegation::This {
                    target,
                    arguments: hir::ConstructorArguments {
                        locals: Arena::new(),
                        statements: Vec::new(),
                        args: Vec::new(),
                    },
                },
                body: hir::Body {
                    locals: Arena::new(),
                    statements: Vec::new(),
                },
            },
            span: SPAN,
            origin: definition_origin(),
        });
        self.classes[illegal_state_class]
            .constructors
            .push(message_constructor);
        hir::CompilerExceptionCore {
            throwable: self.exception_target("Throwable", include),
            unwrap_exception: self.exception_target("UnwrapException", include),
            class_cast_exception: self.exception_target("ClassCastException", include),
            arithmetic_exception: self.exception_target("ArithmeticException", include),
            index_out_of_bounds_exception: self
                .exception_target("IndexOutOfBoundsException", include),
            illegal_state_exception,
            illegal_state_message_constructor: hir::MessageClassConstructor {
                class: illegal_state_class,
                constructor: message_constructor,
            },
        }
    }
}
