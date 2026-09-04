use super::super::*;

impl Harness {
    pub(in crate::tests) fn declare_interface(
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
            access: hir::NominalAccess::public(),
            self_application,
            type_params,
            parents: Vec::new(),
            methods: Vec::new(),
            private_methods: Vec::new(),
            properties: Vec::new(),
            span: SPAN,
        });
        let actual = self.interface_application(interface, self_arguments);
        assert_eq!(actual, self_application);
        for method in methods {
            self.add_interface_method_signature(interface, method);
        }
        interface
    }

    pub(in crate::tests) fn add_interface_method_signature(
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
            access: hir::DeclarationAccess::public(),
            override_access: Vec::new(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: method.is_suspend,
            modifiers: hir::CallableModifiers::default(),
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
            role: hir::InterfaceMemberRole::Function,
            implementation: hir::InterfaceMemberImplementation::AbstractSlot,
            overrides: Vec::new(),
        });
        self.functions[function].method.as_mut().unwrap().dispatch =
            hir::MethodDispatch::Interface(member);
        self.interfaces[interface].methods.push(member);
    }

    pub(in crate::tests) fn interface(&mut self, name: &str, methods: &[&str]) -> hir::InterfaceId {
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
}
