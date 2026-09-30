use super::*;

impl Harness {
    pub(super) fn user_fn(&mut self, name: &str, body: hir::Body) -> hir::FunctionId {
        let unit = self.unit;
        self.user_fn_full(name, Vec::new(), Vec::new(), unit, body)
    }

    pub(super) fn register_generic(
        &mut self,
        function: hir::FunctionId,
        parameters: Vec<hir::TypeParamDecl>,
    ) -> hir::GenericFunctionId {
        if let hir::FunctionGenericity::Generic {
            definition,
            parameters: existing,
        } = &self.functions[function].genericity
        {
            assert_eq!(existing, &parameters);
            return *definition;
        }
        let generic = self.generic_functions.alloc(hir::GenericFunction {
            function,
            no_gc_type_params: Vec::new(),
            gc_free_pointee_requirements: Vec::new(),
        });
        self.functions[function].genericity = hir::FunctionGenericity::Generic {
            definition: generic,
            parameters,
        };
        generic
    }

    pub(super) fn user_fn_full(
        &mut self,
        name: &str,
        type_params: Vec<String>,
        params: Vec<hir::Param>,
        return_ty: hir::TypeId,
        body: hir::Body,
    ) -> hir::FunctionId {
        let generic = !type_params.is_empty();
        let type_params = type_params.into_iter().map(type_param).collect();
        let id = self.functions.alloc(hir::Function {
            name: name.to_string(),
            access: hir::DeclarationAccess::public(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            modifiers: hir::CallableModifiers::default(),
            params,
            return_ty,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::User(body),
            method: None,
            span: SPAN,
        });
        if generic {
            self.register_generic(id, type_params);
        }
        self.top_level.push(id);
        id
    }

    pub(super) fn instantiate(
        &mut self,
        function: hir::FunctionId,
        type_args: Vec<hir::TypeId>,
    ) -> hir::ResolvedGenericFunctionId {
        let Some(generic) = self.functions[function].generic_definition() else {
            panic!("generic test function must be registered")
        };
        if let Some((id, _)) = self
            .instantiations
            .iter()
            .find(|(_, resolved)| resolved.generic == generic && resolved.type_args == type_args)
        {
            return id;
        }
        self.instantiations
            .alloc(hir::ResolvedGenericFunction { generic, type_args })
    }

    pub(super) fn test_coroutine_core(&mut self, throwable: hir::ClassId) -> hir::CoroutineCore {
        let throwable_ty = self.class_ty(throwable);
        let t = self
            .types
            .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
        let type_param = || hir::TypeParamDecl {
            id: hir::TypeParamId::from_raw(0),
            name: "T".to_string(),
            bounds: hir::TypeParamBounds::Unconstrained,
            span: SPAN,
        };
        let continuation =
            self.declare_interface("Continuation", vec![type_param()], vec![t], Vec::new());
        let continuation_ty = self.interface_applications
            [self.interfaces[continuation].self_application]
            .canonical_type;
        let mut resume_locals = Arena::new();
        let resume_receiver = resume_locals.alloc(local("this", continuation_ty));
        let resume_value = resume_locals.alloc(local("value", t));
        let continuation_resume = self.functions.alloc(hir::Function {
            name: "Continuation.resume".to_string(),
            access: hir::DeclarationAccess::public(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            modifiers: hir::CallableModifiers::default(),
            params: vec![
                param("this", continuation_ty, resume_receiver),
                param("value", t, resume_value),
            ],
            return_ty: self.unit,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::Abstract {
                locals: resume_locals,
            },
            method: Some(hir::Method {
                owner: continuation_ty,
                modifier: hir::MethodModifier::Abstract,
                dispatch: hir::MethodDispatch::Direct,
            }),
            span: SPAN,
        });
        let mut failure_locals = Arena::new();
        let failure_receiver = failure_locals.alloc(local("this", continuation_ty));
        let failure = failure_locals.alloc(local("exception", throwable_ty));
        let continuation_resume_with_exception = self.functions.alloc(hir::Function {
            name: "Continuation.resumeWithException".to_string(),
            access: hir::DeclarationAccess::public(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            modifiers: hir::CallableModifiers::default(),
            params: vec![
                param("this", continuation_ty, failure_receiver),
                param("exception", throwable_ty, failure),
            ],
            return_ty: self.unit,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::Abstract {
                locals: failure_locals,
            },
            method: Some(hir::Method {
                owner: continuation_ty,
                modifier: hir::MethodModifier::Abstract,
                dispatch: hir::MethodDispatch::Direct,
            }),
            span: SPAN,
        });
        for method in [
            hir::MethodSig {
                name: "resume".to_string(),
                is_suspend: false,
                attributes: hir::FunctionAttributes::default(),
                type_params: Vec::new(),
                params: vec![param("value", t, resume_value)],
                return_ty: self.unit,
                span: SPAN,
            },
            hir::MethodSig {
                name: "resumeWithException".to_string(),
                is_suspend: false,
                attributes: hir::FunctionAttributes::default(),
                type_params: Vec::new(),
                params: vec![param("exception", throwable_ty, failure)],
                return_ty: self.unit,
                span: SPAN,
            },
        ] {
            self.add_interface_method_signature(continuation, method);
        }
        self.functions[continuation_resume]
            .method
            .as_mut()
            .expect("compiler-core declarations are interface methods")
            .dispatch = hir::MethodDispatch::Interface(self.interfaces[continuation].methods[0]);
        self.functions[continuation_resume_with_exception]
            .method
            .as_mut()
            .expect("compiler-core declarations are interface methods")
            .dispatch = hir::MethodDispatch::Interface(self.interfaces[continuation].methods[1]);

        let suspend_task = self.declare_interface(
            "SuspendTask",
            vec![type_param()],
            vec![t],
            vec![hir::MethodSig {
                name: "run".to_string(),
                is_suspend: true,
                attributes: hir::FunctionAttributes::default(),
                type_params: Vec::new(),
                params: Vec::new(),
                return_ty: t,
                span: SPAN,
            }],
        );
        let suspend_task_ty = self.interface_applications
            [self.interfaces[suspend_task].self_application]
            .canonical_type;
        let mut run_locals = Arena::new();
        let run_receiver = run_locals.alloc(local("this", suspend_task_ty));
        let suspend_task_run = self.functions.alloc(hir::Function {
            name: "SuspendTask.run".to_string(),
            access: hir::DeclarationAccess::public(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: true,
            modifiers: hir::CallableModifiers::default(),
            params: vec![param("this", suspend_task_ty, run_receiver)],
            return_ty: t,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::Abstract { locals: run_locals },
            method: Some(hir::Method {
                owner: suspend_task_ty,
                modifier: hir::MethodModifier::Abstract,
                dispatch: hir::MethodDispatch::Interface(self.interfaces[suspend_task].methods[0]),
            }),
            span: SPAN,
        });

        let suspend_registration = self.declare_interface(
            "SuspendRegistration",
            vec![type_param()],
            vec![t],
            vec![hir::MethodSig {
                name: "register".to_string(),
                is_suspend: false,
                attributes: hir::FunctionAttributes::default(),
                type_params: Vec::new(),
                params: Vec::new(),
                return_ty: self.unit,
                span: SPAN,
            }],
        );
        let suspend_registration_ty = self.interface_applications
            [self.interfaces[suspend_registration].self_application]
            .canonical_type;
        let mut register_locals = Arena::new();
        let register_receiver = register_locals.alloc(local("this", suspend_registration_ty));
        let suspend_registration_register = self.functions.alloc(hir::Function {
            name: "SuspendRegistration.register".to_string(),
            access: hir::DeclarationAccess::public(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            modifiers: hir::CallableModifiers::default(),
            params: vec![param("this", suspend_registration_ty, register_receiver)],
            return_ty: self.unit,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::Abstract {
                locals: register_locals,
            },
            method: Some(hir::Method {
                owner: suspend_registration_ty,
                modifier: hir::MethodModifier::Abstract,
                dispatch: hir::MethodDispatch::Interface(
                    self.interfaces[suspend_registration].methods[0],
                ),
            }),
            span: SPAN,
        });

        for function in [
            continuation_resume,
            continuation_resume_with_exception,
            suspend_task_run,
            suspend_registration_register,
        ] {
            self.functions[function].genericity =
                hir::FunctionGenericity::OwnerParameterizedMethod {
                    owner_parameters: vec![type_param()],
                    no_gc_type_params: Vec::new(),
                    gc_free_pointee_requirements: Vec::new(),
                };
        }
        let start_coroutine = self.functions.alloc(hir::Function {
            name: "startCoroutine".to_string(),
            access: hir::DeclarationAccess::public(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            modifiers: hir::CallableModifiers::default(),
            params: Vec::new(),
            return_ty: self.unit,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
                kind: hir::IntrinsicFunctionKind::CoroutineStart,
                provider: hir::IntrinsicProviderId::from_raw(0),
            }),
            method: None,
            span: SPAN,
        });
        let suspend_coroutine = self.functions.alloc(hir::Function {
            name: "suspendCoroutine".to_string(),
            access: hir::DeclarationAccess::public(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: true,
            modifiers: hir::CallableModifiers::default(),
            params: Vec::new(),
            return_ty: t,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
                kind: hir::IntrinsicFunctionKind::CoroutineSuspend,
                provider: hir::IntrinsicProviderId::from_raw(0),
            }),
            method: None,
            span: SPAN,
        });
        for function in [start_coroutine, suspend_coroutine] {
            self.register_generic(function, vec![type_param()]);
            self.top_level.push(function);
        }
        hir::CoroutineCore {
            continuation,
            continuation_resume,
            continuation_resume_with_exception,
            suspend_task,
            suspend_task_run,
            suspend_registration,
            suspend_registration_register,
            start_coroutine,
            suspend_coroutine,
        }
    }

    pub(super) fn test_iteration_core(
        &mut self,
        option_core: hir::OptionCore,
    ) -> hir::IterationCore {
        let parameter = hir::TypeParamDecl {
            id: hir::TypeParamId::with_substitution_slot(u32::MAX, 0),
            name: "T".to_string(),
            bounds: hir::TypeParamBounds::Unconstrained,
            span: SPAN,
        };
        let parameter_ty = self.types.alloc(hir::Type::Param(parameter.id));
        let iterator =
            self.declare_interface("Iterator", vec![parameter], vec![parameter_ty], Vec::new());
        let option_application = self.enum_application(self.option_enum, vec![parameter_ty]);
        let option_ty = self.enum_applications[option_application].canonical_type;
        self.add_interface_method_signature(
            iterator,
            hir::MethodSig {
                name: "next".to_string(),
                is_suspend: false,
                attributes: hir::FunctionAttributes::default(),
                type_params: Vec::new(),
                params: Vec::new(),
                return_ty: option_ty,
                span: SPAN,
            },
        );
        let next = self.interfaces[iterator].methods[0];
        hir::IterationCore::checked(
            &self.interfaces,
            &self.interface_applications,
            &self.interface_methods,
            &self.functions,
            &self.enums,
            &self.enum_applications,
            &self.types,
            option_core,
            iterator,
            next,
        )
        .expect("test Iterator has the core shape")
    }
}
