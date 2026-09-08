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
            override_access: Vec::new(),
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
        let resume_value = resume_locals.alloc(local("value", t));
        let continuation_resume = self.functions.alloc(hir::Function {
            name: "Continuation.resume".to_string(),
            access: hir::DeclarationAccess::public(),
            override_access: Vec::new(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            modifiers: hir::CallableModifiers::default(),
            params: vec![param("value", t, resume_value)],
            return_ty: self.unit,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::User(hir::Body {
                locals: resume_locals,
                statements: Vec::new(),
            }),
            method: Some(hir::Method {
                owner: continuation_ty,
                modifier: hir::MethodModifier::Abstract,
                dispatch: hir::MethodDispatch::Direct,
            }),
            span: SPAN,
        });
        let mut failure_locals = Arena::new();
        let failure = failure_locals.alloc(local("exception", throwable_ty));
        let continuation_resume_with_exception = self.functions.alloc(hir::Function {
            name: "Continuation.resumeWithException".to_string(),
            access: hir::DeclarationAccess::public(),
            override_access: Vec::new(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            modifiers: hir::CallableModifiers::default(),
            params: vec![param("exception", throwable_ty, failure)],
            return_ty: self.unit,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::User(hir::Body {
                locals: failure_locals,
                statements: Vec::new(),
            }),
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
        let suspend_task_run = self.functions.alloc(hir::Function {
            name: "SuspendTask.run".to_string(),
            access: hir::DeclarationAccess::public(),
            override_access: Vec::new(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: true,
            modifiers: hir::CallableModifiers::default(),
            params: Vec::new(),
            return_ty: t,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::User(hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            }),
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
        let suspend_registration_register = self.functions.alloc(hir::Function {
            name: "SuspendRegistration.register".to_string(),
            access: hir::DeclarationAccess::public(),
            override_access: Vec::new(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            modifiers: hir::CallableModifiers::default(),
            params: Vec::new(),
            return_ty: self.unit,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::User(hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            }),
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
            override_access: Vec::new(),
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
            override_access: Vec::new(),
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

    fn test_iteration_core(&mut self, option_core: hir::OptionCore) -> hir::IterationCore {
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

    pub(super) fn finish(self, entry: hir::FunctionId) -> hir::LegacyExecutableExportHir {
        self.finish_with_coroutine_core(entry, false)
    }

    pub(super) fn finish_with_initialization_core(
        mut self,
        entry: hir::FunctionId,
    ) -> hir::LegacyExecutableExportHir {
        self.needs_initialization_core = true;
        self.finish(entry)
    }

    pub(super) fn finish_coroutines(
        self,
        entry: hir::FunctionId,
    ) -> hir::LegacyExecutableExportHir {
        self.finish_with_coroutine_core(entry, true)
    }

    pub(super) fn finish_with_coroutine_core(
        mut self,
        entry: hir::FunctionId,
        include_exceptions: bool,
    ) -> hir::LegacyExecutableExportHir {
        let exception_core = self.test_exception_core(include_exceptions);
        let coroutine_core = self.test_coroutine_core(exception_core.throwable.class());
        let t = self
            .types
            .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
        let ptr = self.declare_struct("Ptr", vec![type_param("T")], vec![t], &[], &[]);
        let fun_ptr = self.declare_struct("FunPtr", vec![type_param("F")], vec![t], &[], &[]);
        let pinned_ptr = self.declare_struct("PinnedPtr", vec![type_param("T")], vec![t], &[], &[]);
        let gc_handle = self.declare_struct("GcHandle", vec![type_param("T")], vec![t], &[], &[]);
        let ffi_core = hir::FfiCore {
            ptr,
            fun_ptr,
            pinned_ptr,
            gc_handle,
            ptr_to_ulong: entry,
            ptr_cast: entry,
            ptr_load: entry,
            ptr_load_offset: entry,
            ptr_store: entry,
            ptr_store_offset: entry,
            ptr_plus: entry,
            ptr_minus: entry,
            address_of: entry,
            size_of: entry,
            align_of: entry,
            gc_pin_raw: entry,
            gc_unpin_raw: entry,
            gc_get_handle_raw: entry,
            gc_release_handle_raw: entry,
        };
        let unit_variants = |variants: &[&str]| {
            variants
                .iter()
                .map(|variant| hir::Variant {
                    name: (*variant).to_string(),
                    fields: Vec::new(),
                })
                .collect()
        };
        let callback_mode = self.declare_enum(
            "ForeignCallbackMode",
            Vec::new(),
            Vec::new(),
            unit_variants(&["Reusable", "OneShot"]),
        );
        let callback_state = self.declare_enum(
            "ForeignCallbackState",
            Vec::new(),
            Vec::new(),
            unit_variants(&["Registered", "Active", "Completed", "Failed"]),
        );
        let integer_types = self.integers;
        let integer_owners = hir::IntegerKind::ALL.map(|kind| {
            self.declare_fixed_intrinsic_struct(
                kind.canonical_name(),
                hir::IntrinsicTypeKind::Integer(kind),
                integer_types.owner(kind),
            )
        });
        let intrinsic_integers = hir::IntegerTypeCore::new(integer_owners)
            .expect("the eight integer kinds receive distinct nominal owners");
        let intrinsic_boolean = self.declare_fixed_intrinsic_struct(
            "Boolean",
            hir::IntrinsicTypeKind::Boolean,
            self.boolean,
        );
        let intrinsic_string = self.declare_intrinsic_class(
            "String",
            hir::IntrinsicTypeKind::String,
            Vec::new(),
            Vec::new(),
            CanonicalTypePlan::Existing(self.string),
        );
        let intrinsic_array = self.intrinsic_array_class(hir::IntrinsicTypeKind::Array);
        let intrinsic_mutable_array =
            self.intrinsic_array_class(hir::IntrinsicTypeKind::MutableArray);
        let mut source_contexts = Arena::new();
        source_contexts.alloc(hir::SourceContext {
            function_name: String::new(),
            type_name: String::new(),
        });
        let option_some = hir::EnumVariantRef::checked(&self.enums, self.option_enum, 0)
            .expect("test Option has Some");
        let option_some_payload = hir::EnumVariantFieldRef::checked(&self.enums, option_some, 0)
            .expect("test Option Some has its payload");
        let option_none = hir::EnumVariantRef::checked(&self.enums, self.option_enum, 1)
            .expect("test Option has None");
        let option_core =
            hir::OptionCore::checked(&self.enums, &self.types, option_some_payload, option_none)
                .expect("test Option has the core shape");
        let iteration_core = self.test_iteration_core(option_core);
        let callback_mode_application = self.enums[callback_mode].self_application;
        let callback_modes = hir::ForeignCallbackModes::checked(
            &self.enums,
            &self.enum_applications,
            hir::AppliedEnumVariantRef::checked_index(
                &self.enums,
                &self.enum_applications,
                callback_mode_application,
                0,
            )
            .expect("test callback mode has Reusable"),
            hir::AppliedEnumVariantRef::checked_index(
                &self.enums,
                &self.enum_applications,
                callback_mode_application,
                1,
            )
            .expect("test callback mode has OneShot"),
        )
        .expect("test callback mode has the core shape");
        let callback_state_application = self.enums[callback_state].self_application;
        let callback_state_ref = |index| {
            hir::AppliedEnumVariantRef::checked_index(
                &self.enums,
                &self.enum_applications,
                callback_state_application,
                index,
            )
            .expect("test callback state variant exists")
        };
        let callback_states = hir::ForeignCallbackStates::checked(
            &self.enums,
            &self.enum_applications,
            callback_state_ref(0),
            callback_state_ref(1),
            callback_state_ref(2),
            callback_state_ref(3),
        )
        .expect("test callback state has the core shape");
        let throwable = self.class_ty(exception_core.throwable.class());
        let callback_failure_application = self.enum_application(self.option_enum, vec![throwable]);
        let callback_failure_some = hir::AppliedEnumVariantRef::checked(
            &self.enums,
            &self.enum_applications,
            callback_failure_application,
            option_core.some(),
        )
        .expect("test callback failure has Some");
        let callback_failure_result = hir::ForeignCallbackFailureResult::checked(
            &self.enums,
            &self.enum_applications,
            option_core,
            throwable,
            hir::AppliedEnumVariantFieldRef::checked(
                &self.enums,
                &self.enum_applications,
                callback_failure_some,
                option_core.some_payload().local_index(),
            )
            .expect("test callback failure Some has a payload"),
            hir::AppliedEnumVariantRef::checked(
                &self.enums,
                &self.enum_applications,
                callback_failure_application,
                option_core.none(),
            )
            .expect("test callback failure has None"),
        )
        .expect("test callback failure has the core shape");
        let module = hir::Module {
            public_surface: hir::PublicSemanticSurface::default(),
            source_files: vec![hir::SourceFileMetadata {
                visibility_source: hir::VisibilitySource::CurrentUnit(
                    scoop_ast::Stage1SourceHandle::new(scoop_ast::Stage1RequestId::from_raw(0), 0),
                ),
                provider: hir::IntrinsicProviderId::from_raw(0),
                name: "<test>".to_string(),
                source: String::new(),
            }],
            source_contexts,
            types: self.types,
            function_types: Arena::new(),
            lambdas: Arena::new(),
            anonymous_functions: Arena::new(),
            local_functions: Arena::new(),
            callable_references: Arena::new(),
            bound_callable_refs: Arena::new(),
            function_coercions: Arena::new(),
            foreign_callback_registrations: Arena::new(),
            source_parameter_interfaces: Vec::new(),
            export_default_exprs: Arena::new(),
            export_default_sources: Arena::new(),
            export_vararg_parameter_types: Arena::new(),
            functions: self.functions,
            extern_functions: self.extern_functions,
            globals: Arena::new(),
            initialization_units: Arena::new(),
            initialization_failure_roots: Arena::new(),
            objects: Arena::new(),
            object_types: Arena::new(),
            companion_relations: Arena::new(),
            singleton_values: Arena::new(),
            singleton_published_roots: Arena::new(),
            properties: self.properties,
            extension_properties: Arena::new(),
            property_getters: self.property_getters,
            property_setters: self.property_setters,
            delegate_storages: Arena::new(),
            type_aliases: Arena::new(),
            generic_functions: self.generic_functions,
            method_applications: self.method_applications,
            generic_methods: self.generic_methods,
            generic_method_applications: self.generic_method_applications,
            derived_equality_applications: Arena::new(),
            structs: self.structs,
            struct_constructors: self.struct_constructors,
            struct_constructor_applications: self.struct_constructor_applications,
            struct_applications: self.struct_applications,
            enums: self.enums,
            enum_applications: self.enum_applications,
            classes: self.classes,
            class_fields: self.class_fields,
            class_constructors: self.class_constructors,
            class_constructor_applications: self.class_constructor_applications,
            class_applications: self.class_applications,
            interfaces: self.interfaces,
            interface_applications: self.interface_applications,
            interface_methods: self.interface_methods,
            top_level: self.top_level,
            unit: self.unit,
            boolean: self.boolean,
            string: self.string,
            option_core,
            iteration_core,
            exception_core,
            coroutine_core,
            ffi_core,
            foreign_callback_core: hir::ForeignCallbackCore {
                callback: ptr,
                modes: callback_modes,
                states: callback_states,
                failure_result: callback_failure_result,
                register: entry,
                retain: entry,
                release: entry,
                query_state: entry,
                failure: entry,
            },
            intrinsic_type_core: hir::IntrinsicTypeCore {
                integers: intrinsic_integers,
                boolean: intrinsic_boolean,
                string: intrinsic_string,
                array: intrinsic_array,
                mutable_array: intrinsic_mutable_array,
                ptr,
                fun_ptr,
            },
            source_location_core: hir::SourceLocationCore {
                location: ptr,
                current: entry,
            },
            instantiations: self.instantiations,
        };
        hir::LegacyExecutableExportHir::try_new(module, entry)
            .expect("the MIR test harness builds a valid legacy executable entry")
    }
}
