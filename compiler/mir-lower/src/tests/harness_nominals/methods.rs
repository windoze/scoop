use super::super::*;

impl Harness {
    /// A member function (kept out of `top_level`, as hir-lower
    /// does); member metadata carries the receiver type. The name is
    /// qualified `Owner.method`, as hir-lower names members.
    pub(in crate::tests) fn method_fn(
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

    pub(in crate::tests) fn inherited_virtual_dispatch(
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

    pub(in crate::tests) fn bind_interface_implementation(
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

    pub(in crate::tests) fn same_method_shape(
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

    pub(in crate::tests) fn method_application(
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
}
