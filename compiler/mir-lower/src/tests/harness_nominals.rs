use super::*;

impl Harness {
    /// `Option<inner>` (core's enum applied to one argument).
    pub(super) fn option(&mut self, inner: hir::TypeId) -> hir::TypeId {
        let application = self.enum_application(self.option_enum, vec![inner]);
        self.enum_applications[application].canonical_type
    }

    pub(super) fn any(&mut self) -> hir::TypeId {
        self.types.alloc(hir::Type::Any)
    }

    pub(super) fn class_ty(&mut self, id: hir::ClassId) -> hir::TypeId {
        let application = self.class_application(id, Vec::new());
        self.class_applications[application].canonical_type
    }

    pub(super) fn interface_ty(&mut self, id: hir::InterfaceId) -> hir::TypeId {
        self.interface_app(id, Vec::new())
    }

    pub(super) fn interface_app(
        &mut self,
        id: hir::InterfaceId,
        arguments: Vec<hir::TypeId>,
    ) -> hir::TypeId {
        assert_eq!(self.interfaces[id].type_params.len(), arguments.len());
        let application = self.interface_application(id, arguments);
        self.interface_applications[application].canonical_type
    }

    pub(super) fn struct_ty(&mut self, id: hir::StructId) -> hir::TypeId {
        self.struct_app(id, Vec::new())
    }

    pub(super) fn enum_ty(&mut self, id: hir::EnumId) -> hir::TypeId {
        assert!(self.enums[id].type_params.is_empty());
        let application = self.enum_application(id, Vec::new());
        self.enum_applications[application].canonical_type
    }

    pub(super) fn struct_application_of(&self, ty: hir::TypeId) -> hir::StructApplicationId {
        let hir::Type::Struct(application) = self.types[ty] else {
            panic!("expected a struct application type")
        };
        application
    }

    pub(super) fn enum_application_of(&self, ty: hir::TypeId) -> hir::EnumApplicationId {
        let hir::Type::Enum(application) = self.types[ty] else {
            panic!("expected an enum application type")
        };
        application
    }

    pub(super) fn class_application_of(&self, ty: hir::TypeId) -> hir::ClassApplicationId {
        let hir::Type::Class(application) = self.types[ty] else {
            panic!("expected a class application type")
        };
        application
    }

    pub(super) fn struct_application(
        &mut self,
        template: hir::StructId,
        arguments: Vec<hir::TypeId>,
    ) -> hir::StructApplicationId {
        let key = (template, arguments);
        if let Some(application) = self.struct_applications_by_key.get(&key) {
            return *application;
        }
        let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
        let application = self.struct_applications.alloc(hir::StructApplication {
            template,
            arguments: key.1.clone(),
            canonical_type,
            representation: hir::StructApplicationRepresentation::Declared,
        });
        let actual_type = self.types.alloc(hir::Type::Struct(application));
        assert_eq!(actual_type, canonical_type);
        self.struct_applications_by_key.insert(key, application);
        application
    }

    pub(super) fn enum_application(
        &mut self,
        template: hir::EnumId,
        arguments: Vec<hir::TypeId>,
    ) -> hir::EnumApplicationId {
        let key = (template, arguments);
        if let Some(application) = self.enum_applications_by_key.get(&key) {
            return *application;
        }
        let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
        let application = self.enum_applications.alloc(hir::EnumApplication {
            template,
            arguments: key.1.clone(),
            canonical_type,
        });
        let actual_type = self.types.alloc(hir::Type::Enum(application));
        assert_eq!(actual_type, canonical_type);
        self.enum_applications_by_key.insert(key, application);
        application
    }

    pub(super) fn declare_enum(
        &mut self,
        name: &str,
        type_params: Vec<hir::TypeParamDecl>,
        self_arguments: Vec<hir::TypeId>,
        variants: Vec<hir::Variant>,
    ) -> hir::EnumId {
        assert_eq!(type_params.len(), self_arguments.len());
        let self_application =
            hir::EnumApplicationId::from_raw((self.enum_applications.len() as u32).into());
        let enumeration = self.enums.alloc(hir::EnumDecl {
            name: name.to_string(),
            self_application,
            type_params,
            no_gc: false,
            variants,
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            derived_equality: None,
            span: SPAN,
        });
        let actual = self.enum_application(enumeration, self_arguments);
        assert_eq!(actual, self_application);
        enumeration
    }

    pub(super) fn class_application(
        &mut self,
        template: hir::ClassId,
        arguments: Vec<hir::TypeId>,
    ) -> hir::ClassApplicationId {
        let key = (template, arguments);
        if let Some(application) = self.class_applications_by_key.get(&key) {
            return *application;
        }
        let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
        let representation = match self.classes[template].representation {
            hir::ClassRepresentation::Declared(_) => hir::ClassApplicationRepresentation::Declared,
            hir::ClassRepresentation::Intrinsic(declaration) => {
                hir::ClassApplicationRepresentation::Intrinsic(declaration.kind.application(&key.1))
            }
        };
        let application = self.class_applications.alloc(hir::ClassApplication {
            template,
            arguments: key.1.clone(),
            canonical_type,
            representation,
        });
        let actual_type = self.types.alloc(hir::Type::Class(application));
        assert_eq!(actual_type, canonical_type);
        self.class_applications_by_key.insert(key, application);
        application
    }

    pub(super) fn interface_application(
        &mut self,
        template: hir::InterfaceId,
        arguments: Vec<hir::TypeId>,
    ) -> hir::InterfaceApplicationId {
        let key = (template, arguments);
        if let Some(application) = self.interface_applications_by_key.get(&key) {
            return *application;
        }
        let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
        let application = self
            .interface_applications
            .alloc(hir::InterfaceApplication {
                template,
                arguments: key.1.clone(),
                canonical_type,
            });
        let actual_type = self.types.alloc(hir::Type::Interface(application));
        assert_eq!(actual_type, canonical_type);
        self.interface_applications_by_key.insert(key, application);
        application
    }

    pub(super) fn declare_interface(
        &mut self,
        name: &str,
        type_params: Vec<hir::TypeParamDecl>,
        self_arguments: Vec<hir::TypeId>,
        methods: Vec<hir::MethodSig>,
    ) -> hir::InterfaceId {
        assert_eq!(type_params.len(), self_arguments.len());
        let self_application = hir::InterfaceApplicationId::from_raw(
            (self.interface_applications.len() as u32).into(),
        );
        let interface = self.interfaces.alloc(hir::InterfaceDecl {
            name: name.to_string(),
            self_application,
            type_params,
            parents: Vec::new(),
            methods: Vec::new(),
            span: SPAN,
        });
        let actual = self.interface_application(interface, self_arguments);
        assert_eq!(actual, self_application);
        for method in methods {
            self.add_interface_method_signature(interface, method);
        }
        interface
    }

    pub(super) fn add_interface_method_signature(
        &mut self,
        interface: hir::InterfaceId,
        method: hir::MethodSig,
    ) {
        assert!(method.type_params.is_empty());
        let declaration = self.interfaces[interface].clone();
        let owner = self.interface_applications[declaration.self_application].canonical_type;
        let mut locals = Arena::new();
        let this = locals.alloc(local("this", owner));
        let mut params = vec![param("this", owner, this)];
        for source in method.params {
            let local = locals.alloc(local(&source.name, source.ty));
            params.push(param(&source.name, source.ty, local));
        }
        let function = self.functions.alloc(hir::Function {
            name: format!("{}.{}", declaration.name, method.name),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: method.is_suspend,
            params,
            return_ty: method.return_ty,
            attributes: method.attributes,
            kind: hir::FunctionKind::User(hir::Body {
                locals,
                statements: Vec::new(),
            }),
            method: Some(hir::Method {
                owner,
                modifier: hir::MethodModifier::Abstract,
                dispatch: hir::MethodDispatch::Direct,
                operator: None,
            }),
            span: method.span,
        });
        if !declaration.type_params.is_empty() {
            self.functions[function].genericity =
                hir::FunctionGenericity::OwnerParameterizedMethod {
                    owner_parameters: declaration.type_params,
                    no_gc_type_params: Vec::new(),
                };
        }
        let member = self.interface_methods.alloc(hir::InterfaceMethod {
            owner: interface,
            function,
        });
        self.functions[function].method.as_mut().unwrap().dispatch =
            hir::MethodDispatch::Interface(member);
        self.interfaces[interface].methods.push(member);
    }

    pub(super) fn interface(&mut self, name: &str, methods: &[&str]) -> hir::InterfaceId {
        let unit = self.unit;
        let methods = methods
            .iter()
            .map(|name| hir::MethodSig {
                name: name.to_string(),
                is_suspend: false,
                attributes: hir::FunctionAttributes::default(),
                type_params: Vec::new(),
                params: Vec::new(),
                return_ty: unit,
                span: SPAN,
            })
            .collect();
        self.declare_interface(name, Vec::new(), Vec::new(), methods)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn class(
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
        let base = base.map(|(base, arguments)| (self.class_ty(base), arguments));
        self.declare_class(name, modifier, constructor, base, interfaces)
    }

    pub(super) fn declare_class(
        &mut self,
        name: &str,
        modifier: hir::ClassModifier,
        constructor: &[(&str, hir::TypeId)],
        base_class: Option<(hir::TypeId, Vec<hir::Expr>)>,
        interfaces: Vec<hir::TypeId>,
    ) -> hir::ClassId {
        let mut interface_implementations = base_class
            .as_ref()
            .map(|(base, _)| {
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
        let class = self.classes.alloc(hir::ClassDecl {
            modifier,
            name: name.to_string(),
            self_application,
            type_params: Vec::new(),
            representation: hir::ClassRepresentation::Declared(
                constructor
                    .iter()
                    .enumerate()
                    .map(|(index, (name, ty))| hir::ConstructorField {
                        parameter: hir::ConstructorParamId::from_raw(index as u32),
                        name: name.to_string(),
                        ty: *ty,
                        mutable: false,
                    })
                    .collect(),
            ),
            base_class,
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
    pub(super) fn exception(&mut self, name: &str) -> hir::ClassId {
        self.class(name, hir::ClassModifier::Final, &[], None, &[])
    }

    pub(super) fn exception_target(&mut self, name: &str, include: bool) -> hir::CompilerException {
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
            constructor: hir::ZeroArgClassConstructor { class },
        }
    }

    pub(super) fn test_exception_core(&mut self, include: bool) -> hir::CompilerExceptionCore {
        hir::CompilerExceptionCore {
            throwable: self.exception_target("Throwable", include),
            unwrap_exception: self.exception_target("UnwrapException", include),
            class_cast_exception: self.exception_target("ClassCastException", include),
            arithmetic_exception: self.exception_target("ArithmeticException", include),
            index_out_of_bounds_exception: self
                .exception_target("IndexOutOfBoundsException", include),
            illegal_state_exception: self.exception_target("IllegalStateException", include),
        }
    }

    /// A member function (kept out of `top_level`, as hir-lower
    /// does); member metadata carries the receiver type. The name is
    /// qualified `Owner.method`, as hir-lower names members.
    pub(super) fn method_fn(
        &mut self,
        name: &str,
        method_of: hir::TypeId,
        params: Vec<hir::Param>,
        return_ty: hir::TypeId,
        body: hir::Body,
    ) -> hir::FunctionId {
        let genericity = match self.types[method_of] {
            hir::Type::Class(application) => {
                let parameters = self.classes[self.class_applications[application].template]
                    .type_params
                    .clone();
                if parameters.is_empty() {
                    hir::FunctionGenericity::Plain
                } else {
                    hir::FunctionGenericity::OwnerParameterizedMethod {
                        owner_parameters: parameters,
                        no_gc_type_params: Vec::new(),
                    }
                }
            }
            hir::Type::Struct(application) => {
                let parameters = self.structs[self.struct_applications[application].template]
                    .type_params
                    .clone();
                if parameters.is_empty() {
                    hir::FunctionGenericity::Plain
                } else {
                    hir::FunctionGenericity::OwnerParameterizedMethod {
                        owner_parameters: parameters,
                        no_gc_type_params: Vec::new(),
                    }
                }
            }
            hir::Type::Enum(application) => {
                let parameters = self.enums[self.enum_applications[application].template]
                    .type_params
                    .clone();
                if parameters.is_empty() {
                    hir::FunctionGenericity::Plain
                } else {
                    hir::FunctionGenericity::OwnerParameterizedMethod {
                        owner_parameters: parameters,
                        no_gc_type_params: Vec::new(),
                    }
                }
            }
            hir::Type::Interface(application) => {
                let parameters = self.interfaces[self.interface_applications[application].template]
                    .type_params
                    .clone();
                if parameters.is_empty() {
                    hir::FunctionGenericity::Plain
                } else {
                    hir::FunctionGenericity::OwnerParameterizedMethod {
                        owner_parameters: parameters,
                        no_gc_type_params: Vec::new(),
                    }
                }
            }
            hir::Type::Any => hir::FunctionGenericity::Plain,
            _ => panic!("test harness methods have nominal owners"),
        };
        let dispatch = match self.types[method_of] {
            hir::Type::Class(application) => {
                let class = self.class_applications[application].template;
                self.inherited_virtual_dispatch(class, name, &params, return_ty)
                    .unwrap_or_else(|| {
                        hir::MethodDispatch::Virtual(hir::VirtualMethodId::from_raw(
                            self.functions.len() as u32,
                        ))
                    })
            }
            hir::Type::Interface(application) => {
                let interface = self.interface_applications[application].template;
                let member = self.interfaces[interface]
                    .methods
                    .iter()
                    .copied()
                    .find(|member| {
                        self.same_method_shape(
                            self.interface_methods[*member].function,
                            name,
                            &params,
                            return_ty,
                        )
                    })
                    .expect("test interface method names an existing typed member");
                hir::MethodDispatch::Interface(member)
            }
            _ => hir::MethodDispatch::Direct,
        };
        let function = self.functions.alloc(hir::Function {
            name: name.to_string(),
            genericity,
            is_suspend: false,
            params,
            return_ty,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::User(body),
            method: Some(hir::Method {
                owner: method_of,
                modifier: hir::MethodModifier::Open,
                dispatch,
                operator: None,
            }),
            span: SPAN,
        });
        match self.types[method_of] {
            hir::Type::Class(application) => self.classes
                [self.class_applications[application].template]
                .methods
                .push(function),
            hir::Type::Struct(application) => self.structs
                [self.struct_applications[application].template]
                .methods
                .push(function),
            hir::Type::Enum(application) => self.enums
                [self.enum_applications[application].template]
                .methods
                .push(function),
            hir::Type::Interface(_) | hir::Type::Any => {}
            _ => unreachable!(),
        }
        self.bind_interface_implementation(method_of, function);
        function
    }

    pub(super) fn inherited_virtual_dispatch(
        &self,
        class: hir::ClassId,
        name: &str,
        params: &[hir::Param],
        return_ty: hir::TypeId,
    ) -> Option<hir::MethodDispatch> {
        let mut base = self.classes[class].base_class.as_ref().map(|(base, _)| {
            let hir::Type::Class(application) = self.types[*base] else {
                panic!("test harness class bases are class applications")
            };
            self.class_applications[application].template
        });
        while let Some(class) = base {
            if let Some(dispatch) = self.classes[class]
                .methods
                .iter()
                .copied()
                .find_map(|method| {
                    self.same_method_shape(method, name, params, return_ty)
                        .then_some(self.functions[method].method?.dispatch)
                })
            {
                return Some(dispatch);
            }
            base = self.classes[class].base_class.as_ref().map(|(base, _)| {
                let hir::Type::Class(application) = self.types[*base] else {
                    panic!("test harness class bases are class applications")
                };
                self.class_applications[application].template
            });
        }
        None
    }

    pub(super) fn bind_interface_implementation(
        &mut self,
        owner: hir::TypeId,
        function: hir::FunctionId,
    ) {
        let implementations = match self.types[owner] {
            hir::Type::Class(application) => {
                let owner = self.class_applications[application].template;
                self.classes[owner].interface_implementations.clone()
            }
            hir::Type::Struct(application) => {
                let owner = self.struct_applications[application].template;
                self.structs[owner].interface_implementations.clone()
            }
            hir::Type::Enum(application) => {
                let owner = self.enum_applications[application].template;
                self.enums[owner].interface_implementations.clone()
            }
            hir::Type::Interface(_) | hir::Type::Any => return,
            _ => unreachable!("test harness methods have nominal owners"),
        };
        let mut matches = Vec::new();
        for (implementation_index, implementation) in implementations.iter().enumerate() {
            for (method_index, implementation_method) in implementation.methods.iter().enumerate() {
                let declaration = self.interface_methods[implementation_method.member].function;
                if self.same_method_shape(
                    declaration,
                    &self.functions[function].name,
                    &self.functions[function].params,
                    self.functions[function].return_ty,
                ) {
                    matches.push((implementation_index, method_index));
                }
            }
        }
        if matches.is_empty() {
            return;
        }
        let application = self.method_application(function);
        let target = hir::InterfaceImplementationTarget::Method(application);
        match self.types[owner] {
            hir::Type::Class(owner_application) => {
                let owner = self.class_applications[owner_application].template;
                for (implementation, method) in matches {
                    self.classes[owner].interface_implementations[implementation].methods[method]
                        .target = target;
                }
            }
            hir::Type::Struct(owner_application) => {
                let owner = self.struct_applications[owner_application].template;
                for (implementation, method) in matches {
                    self.structs[owner].interface_implementations[implementation].methods[method]
                        .target = target;
                }
            }
            hir::Type::Enum(owner_application) => {
                let owner = self.enum_applications[owner_application].template;
                for (implementation, method) in matches {
                    self.enums[owner].interface_implementations[implementation].methods[method]
                        .target = target;
                }
            }
            _ => unreachable!(),
        }
    }

    pub(super) fn same_method_shape(
        &self,
        candidate: hir::FunctionId,
        name: &str,
        params: &[hir::Param],
        return_ty: hir::TypeId,
    ) -> bool {
        let candidate = &self.functions[candidate];
        candidate.name.rsplit('.').next() == name.rsplit('.').next()
            && candidate.params.len() == params.len()
            && candidate
                .params
                .iter()
                .skip(1)
                .zip(params.iter().skip(1))
                .all(|(left, right)| left.ty == right.ty)
            && candidate.return_ty == return_ty
    }

    pub(super) fn method_application(
        &mut self,
        function: hir::FunctionId,
    ) -> hir::MethodApplicationId {
        let owner_ty = self.functions[function]
            .method
            .expect("test harness method has metadata")
            .owner;
        let owner = match self.types[owner_ty] {
            hir::Type::Class(application) => hir::MethodOwnerApplication::Class(application),
            hir::Type::Struct(application) => hir::MethodOwnerApplication::Struct(application),
            hir::Type::Enum(application) => hir::MethodOwnerApplication::Enum(application),
            hir::Type::Interface(application) => {
                hir::MethodOwnerApplication::Interface(application)
            }
            hir::Type::Any => panic!("Any has no methods"),
            _ => panic!("test harness methods have nominal owners"),
        };
        let key = (function, owner);
        if let Some(&application) = self.method_applications_by_key.get(&key) {
            return application;
        }
        let application = self
            .method_applications
            .alloc(hir::MethodApplication { function, owner });
        self.method_applications_by_key.insert(key, application);
        application
    }

    pub(super) fn strukt(&mut self, name: &str, fields: &[(&str, hir::TypeId)]) -> hir::StructId {
        self.strukt_with(name, fields, &[])
    }

    pub(super) fn strukt_with(
        &mut self,
        name: &str,
        fields: &[(&str, hir::TypeId)],
        interfaces: &[hir::InterfaceId],
    ) -> hir::StructId {
        self.declare_struct(name, Vec::new(), Vec::new(), fields, interfaces)
    }

    pub(super) fn declare_struct(
        &mut self,
        name: &str,
        type_params: Vec<hir::TypeParamDecl>,
        self_arguments: Vec<hir::TypeId>,
        fields: &[(&str, hir::TypeId)],
        interfaces: &[hir::InterfaceId],
    ) -> hir::StructId {
        assert_eq!(type_params.len(), self_arguments.len());
        let interfaces: Vec<_> = interfaces
            .iter()
            .map(|&interface| self.interface_ty(interface))
            .collect();
        let interface_implementations = self.interface_implementation_shells(&interfaces);
        let self_application =
            hir::StructApplicationId::from_raw((self.struct_applications.len() as u32).into());
        let strukt = self.structs.alloc(hir::StructDecl {
            name: name.to_string(),
            self_application,
            type_params,
            attributes: hir::StructAttributes::default(),
            representation: hir::StructRepresentation::Declared(
                fields
                    .iter()
                    .map(|(name, ty)| hir::Field {
                        name: name.to_string(),
                        ty: *ty,
                    })
                    .collect(),
            ),
            interfaces,
            interface_implementations,
            methods: Vec::new(),
            derived_equality: None,
            span: SPAN,
        });
        let actual = self.struct_application(strukt, self_arguments);
        assert_eq!(actual, self_application);
        strukt
    }

    pub(super) fn declare_fixed_intrinsic_struct(
        &mut self,
        name: &str,
        kind: hir::IntrinsicTypeKind,
        canonical_type: hir::TypeId,
    ) -> hir::StructId {
        let self_application =
            hir::StructApplicationId::from_raw((self.struct_applications.len() as u32).into());
        let declaration = hir::IntrinsicTypeDeclaration {
            kind,
            provider: hir::IntrinsicProviderId::from_raw(0),
        };
        let strukt = self.structs.alloc(hir::StructDecl {
            name: name.to_string(),
            self_application,
            type_params: Vec::new(),
            attributes: hir::StructAttributes::default(),
            representation: hir::StructRepresentation::Intrinsic(declaration),
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            derived_equality: None,
            span: SPAN,
        });
        let representation = kind.application(&[]);
        let actual = self.struct_applications.alloc(hir::StructApplication {
            template: strukt,
            arguments: Vec::new(),
            canonical_type,
            representation: hir::StructApplicationRepresentation::Intrinsic(representation),
        });
        assert_eq!(actual, self_application);
        self.struct_applications_by_key
            .insert((strukt, Vec::new()), actual);
        strukt
    }

    pub(super) fn declare_intrinsic_class(
        &mut self,
        name: &str,
        kind: hir::IntrinsicTypeKind,
        type_params: Vec<hir::TypeParamDecl>,
        self_arguments: Vec<hir::TypeId>,
        canonical_type_plan: CanonicalTypePlan,
    ) -> hir::ClassId {
        let self_application =
            hir::ClassApplicationId::from_raw((self.class_applications.len() as u32).into());
        let declaration = hir::IntrinsicTypeDeclaration {
            kind,
            provider: hir::IntrinsicProviderId::from_raw(0),
        };
        let class = self.classes.alloc(hir::ClassDecl {
            modifier: hir::ClassModifier::Final,
            name: name.to_string(),
            self_application,
            type_params,
            representation: hir::ClassRepresentation::Intrinsic(declaration),
            base_class: None,
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            span: SPAN,
        });
        let representation = kind.application(&self_arguments);
        let (canonical_type, allocate_canonical_type) = match canonical_type_plan {
            CanonicalTypePlan::Existing(canonical_type) => (canonical_type, false),
            CanonicalTypePlan::Allocate => (
                hir::TypeId::from_raw((self.types.len() as u32).into()),
                true,
            ),
        };
        let actual = self.class_applications.alloc(hir::ClassApplication {
            template: class,
            arguments: self_arguments.clone(),
            canonical_type,
            representation: hir::ClassApplicationRepresentation::Intrinsic(representation),
        });
        assert_eq!(actual, self_application);
        if allocate_canonical_type {
            let allocated = self.types.alloc(hir::Type::Class(actual));
            assert_eq!(allocated, canonical_type);
        }
        self.class_applications_by_key
            .insert((class, self_arguments), actual);
        class
    }

    pub(super) fn interface_implementation_shells(
        &self,
        interfaces: &[hir::TypeId],
    ) -> Vec<hir::InterfaceImplementation> {
        interfaces
            .iter()
            .map(|&interface| {
                let hir::Type::Interface(application) = self.types[interface] else {
                    panic!("test harness interface lists are fully applied")
                };
                let template = self.interface_applications[application].template;
                hir::InterfaceImplementation {
                    interface: application,
                    methods: self.interfaces[template]
                        .methods
                        .iter()
                        .map(|&member| hir::InterfaceMethodImplementation {
                            member,
                            target: hir::InterfaceImplementationTarget::Subclass,
                        })
                        .collect(),
                }
            })
            .collect()
    }
}
