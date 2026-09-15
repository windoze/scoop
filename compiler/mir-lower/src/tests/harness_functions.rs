use super::harness_nominals::test_nominal_identities_without_objects;
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
        let resume_receiver = resume_locals.alloc(local("this", continuation_ty));
        let resume_value = resume_locals.alloc(local("value", t));
        let continuation_resume = self.functions.alloc(hir::Function {
            name: "Continuation.resume".to_string(),
            access: hir::DeclarationAccess::public(),
            override_access: Vec::new(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            modifiers: hir::CallableModifiers::default(),
            params: vec![
                param("this", continuation_ty, resume_receiver),
                param("value", t, resume_value),
            ],
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
        let failure_receiver = failure_locals.alloc(local("this", continuation_ty));
        let failure = failure_locals.alloc(local("exception", throwable_ty));
        let continuation_resume_with_exception = self.functions.alloc(hir::Function {
            name: "Continuation.resumeWithException".to_string(),
            access: hir::DeclarationAccess::public(),
            override_access: Vec::new(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            modifiers: hir::CallableModifiers::default(),
            params: vec![
                param("this", continuation_ty, failure_receiver),
                param("exception", throwable_ty, failure),
            ],
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
        let mut run_locals = Arena::new();
        let run_receiver = run_locals.alloc(local("this", suspend_task_ty));
        let suspend_task_run = self.functions.alloc(hir::Function {
            name: "SuspendTask.run".to_string(),
            access: hir::DeclarationAccess::public(),
            override_access: Vec::new(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: true,
            modifiers: hir::CallableModifiers::default(),
            params: vec![param("this", suspend_task_ty, run_receiver)],
            return_ty: t,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::User(hir::Body {
                locals: run_locals,
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
        let mut register_locals = Arena::new();
        let register_receiver = register_locals.alloc(local("this", suspend_registration_ty));
        let suspend_registration_register = self.functions.alloc(hir::Function {
            name: "SuspendRegistration.register".to_string(),
            access: hir::DeclarationAccess::public(),
            override_access: Vec::new(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            modifiers: hir::CallableModifiers::default(),
            params: vec![param("this", suspend_registration_ty, register_receiver)],
            return_ty: self.unit,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::User(hir::Body {
                locals: register_locals,
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

    pub(super) fn finish(self, entry: hir::FunctionId) -> hir::ExportHirOutput {
        self.finish_with_coroutine_core(entry, false)
    }

    pub(super) fn finish_with_initialization_core(
        mut self,
        entry: hir::FunctionId,
    ) -> hir::ExportHirOutput {
        self.needs_initialization_core = true;
        self.finish(entry)
    }

    pub(super) fn finish_coroutines(self, entry: hir::FunctionId) -> hir::ExportHirOutput {
        self.finish_with_coroutine_core(entry, true)
    }

    pub(super) fn finish_with_coroutine_core(
        mut self,
        entry: hir::FunctionId,
        include_exceptions: bool,
    ) -> hir::ExportHirOutput {
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
                    style: hir::VariantStyle::Unit,
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
        let intrinsic_type_core = hir::IntrinsicTypeCore {
            integers: intrinsic_integers,
            boolean: intrinsic_boolean,
            string: intrinsic_string,
            array: intrinsic_array,
            mutable_array: intrinsic_mutable_array,
            ptr,
            fun_ptr,
        };
        let mut source_contexts = Arena::new();
        source_contexts.alloc(hir::SourceContext::new(
            scoop_identity::SourceIdentity::single_file(),
            hir::SourceContextSubject::File,
        ));
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
        let nominal_identities = test_nominal_identities_without_objects(
            &self.structs,
            &self.enums,
            &self.classes,
            &self.interfaces,
        );
        let property_identities = crate::tests::harness_nominals::test_property_identities(
            &self.properties,
            &Arena::new(),
        );
        let property_accessor_identities =
            crate::tests::harness_nominals::test_property_accessor_identities(
                &self.properties,
                &property_identities,
                &self.property_getters,
                &self.property_setters,
            );
        let type_alias_identities =
            hir::HirTypeAliasIdentities::from_declarations(&Arena::new(), Vec::new())
                .expect("the MIR test fixture has an empty type-alias arena");
        let enum_member_identities =
            hir::HirEnumMemberIdentities::from_declarations(&self.enums, &nominal_identities)
                .expect("the MIR test fixture enum members have persistent identities");
        let field_identities = hir::HirFieldIdentities::from_declarations(
            &self.structs,
            &self.classes,
            &Arena::new(),
            &self.class_fields,
            &self.properties,
            &Arena::new(),
            &nominal_identities,
            &property_identities,
        )
        .expect("the MIR test fixture fields have persistent identities");
        let object_value_identities = hir::HirObjectValueIdentities::from_declarations(
            &Arena::new(),
            &Arena::new(),
            &Arena::new(),
            &nominal_identities,
        )
        .expect("the MIR test fixture has no object values");
        let initialization_unit_identities =
            hir::HirInitializationUnitIdentities::from_declarations(
                &Arena::new(),
                &Arena::new(),
                &self.functions,
                &Arena::new(),
                &Arena::new(),
                &Arena::new(),
                &Arena::new(),
                &Arena::new(),
                &self.properties,
                &Arena::new(),
                &nominal_identities,
                &property_identities,
            )
            .expect("the MIR test fixture has no initialization units");
        let type_identity_inputs = hir::HirTypeIdentityInputs {
            types: &self.types,
            function_types: &self.function_types,
            structs: &self.structs,
            struct_applications: &self.struct_applications,
            enums: &self.enums,
            enum_applications: &self.enum_applications,
            classes: &self.classes,
            class_applications: &self.class_applications,
            interfaces: &self.interfaces,
            interface_applications: &self.interface_applications,
            objects: &Arena::new(),
            intrinsic_core: &intrinsic_type_core,
            nominal_identities: &nominal_identities,
        };
        let type_identities = hir::HirTypeIdentities::from_types(type_identity_inputs)
            .expect("the MIR test fixture types have persistent identities");
        let constructor_identities = crate::tests::harness_nominals::test_constructor_identities(
            type_identity_inputs,
            &self.struct_constructors,
            &self.class_constructors,
            &self.class_constructor_applications,
        );
        let function_identity_rows = self
            .functions
            .iter()
            .map(|(function, declaration)| {
                let accessor = self
                    .property_getters
                    .iter()
                    .find_map(|(getter, value)| {
                        matches!(
                            value.implementation,
                            hir::PropertyAccessorImplementation::Body(actual)
                                | hir::PropertyAccessorImplementation::AbstractSlot(actual)
                                if actual == function
                        )
                        .then_some(hir::HirPropertyAccessorFunction::Getter(getter))
                    })
                    .or_else(|| {
                        self.property_setters.iter().find_map(|(setter, value)| {
                            matches!(
                                value.implementation,
                                hir::PropertyAccessorImplementation::Body(actual)
                                    | hir::PropertyAccessorImplementation::AbstractSlot(actual)
                                    if actual == function
                            )
                            .then_some(hir::HirPropertyAccessorFunction::Setter(setter))
                        })
                    });
                if let Some(accessor) = accessor {
                    return hir::HirFunctionIdentity::property_accessor(accessor);
                }
                assert!(!matches!(
                    declaration.kind,
                    hir::FunctionKind::DerivedEquality
                ));
                let site = scoop_identity::SourceDeclarationSite::new(
                    scoop_identity::ConeIdentity::SINGLE_FILE,
                    scoop_identity::PackagePath::root(),
                    scoop_identity::DefinitionOwnerChain::top_level(),
                    scoop_identity::DeclarationScope::ConeWide,
                )
                .unwrap();
                let source_name = if function == entry {
                    "main".to_string()
                } else {
                    format!("fixture_function_{}", function.into_raw().into_u32())
                };
                let name = scoop_identity::CanonicalIdentifier::new(&source_name).unwrap();
                let own_type_parameters = match &declaration.genericity {
                    hir::FunctionGenericity::Generic { parameters, .. } => parameters.len(),
                    hir::FunctionGenericity::GenericMethod {
                        method_parameters, ..
                    } => method_parameters.len(),
                    hir::FunctionGenericity::Plain
                    | hir::FunctionGenericity::OwnerParameterizedMethod { .. } => 0,
                };
                let identity = hir::HirSourceFunctionIdentity::from_declaration(
                    scoop_identity::SourceDeclarationKey::function(
                        site,
                        name,
                        u32::try_from(own_type_parameters).unwrap(),
                        None,
                        Vec::new(),
                    ),
                )
                .unwrap();
                hir::HirFunctionIdentity::source(identity)
            })
            .collect();
        let function_identities = hir::HirFunctionIdentities::checked(
            hir::HirFunctionIdentityInputs {
                functions: &self.functions,
                lambdas: &Arena::new(),
                anonymous_functions: &Arena::new(),
                local_functions: &Arena::new(),
                property_getters: &self.property_getters,
                property_setters: &self.property_setters,
                property_accessor_identities: &property_accessor_identities,
                initialization_units: &Arena::new(),
                initialization_unit_identities: &initialization_unit_identities,
                derived_equality_applications: &Arena::new(),
                structs: &self.structs,
                enums: &self.enums,
                type_identities: &type_identities,
                struct_constructors: &self.struct_constructors,
                class_constructors: &self.class_constructors,
                constructor_identities: &constructor_identities,
                enum_member_identities: &enum_member_identities,
            },
            function_identity_rows,
        )
        .expect("the MIR test fixture functions have persistent identities");
        let hir::HirSourceFunctionIdentity::Plain(entry_identity) = function_identities[entry]
            .source_identity()
            .expect("the executable entry is a source function")
        else {
            panic!("the executable entry is non-generic")
        };
        let entry_source = scoop_identity::SourceIdentity::single_file();
        let entry_context = scoop_identity::SourceContextKey::File {
            source: entry_source.clone(),
        };
        let entry_span = self.functions[entry].span;
        let entry_origin = scoop_identity::DefinitionOrigin::new(
            entry_source,
            scoop_identity::SourceSpan::new(u64::from(entry_span.start), u64::from(entry_span.end))
                .unwrap(),
            &entry_context,
        )
        .unwrap();
        let export_definition_origins = hir::HirExportDefinitionOrigins::canonicalize(vec![
            scoop_identity::DefinitionOriginRecord::new(
                scoop_identity::DefinitionOriginSubject::Function(entry_identity.id()),
                entry_origin,
            ),
        ])
        .unwrap();
        let dispatch_key =
            |function: hir::FunctionId, interface: bool| match &function_identities[function] {
                hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(record)) => {
                    if interface {
                        scoop_identity::DispatchSlotKey::interface_method(record.id())
                    } else {
                        scoop_identity::DispatchSlotKey::virtual_method(record.id())
                    }
                }
                hir::HirFunctionIdentity::PropertyAccessor(
                    hir::HirPropertyAccessorFunction::Getter(getter),
                ) => scoop_identity::DispatchSlotKey::property_getter(
                    property_accessor_identities[*getter].id(),
                ),
                hir::HirFunctionIdentity::PropertyAccessor(
                    hir::HirPropertyAccessorFunction::Setter(setter),
                ) => scoop_identity::DispatchSlotKey::property_setter(
                    property_accessor_identities[*setter].id(),
                ),
                _ => panic!("test dispatch slot owner must be a plain function or accessor"),
            };
        let mut virtual_roots = std::collections::BTreeMap::new();
        for (function, declaration) in self.functions.iter() {
            let Some(method) = declaration.method else {
                continue;
            };
            let family = match method.dispatch {
                hir::MethodDispatch::Virtual(family)
                | hir::MethodDispatch::FinalOverride(family) => family,
                hir::MethodDispatch::Direct | hir::MethodDispatch::Interface(_) => continue,
            };
            virtual_roots.entry(family).or_insert(function);
        }
        let virtual_slots = virtual_roots
            .into_iter()
            .map(|(family, root)| {
                let record =
                    scoop_identity::CborIdentityRecord::from_key(dispatch_key(root, false))
                        .unwrap();
                (family, root, record)
            })
            .collect();
        let interface_slots = self
            .interface_methods
            .iter()
            .map(|(_, member)| {
                scoop_identity::CborIdentityRecord::from_key(dispatch_key(member.function, true))
                    .unwrap()
            })
            .collect();
        let dispatch_slot_identities = hir::HirDispatchSlotIdentities::checked(
            hir::HirDispatchSlotIdentityInputs {
                functions: &self.functions,
                function_identities: &function_identities,
                property_accessor_identities: &property_accessor_identities,
                interface_methods: &self.interface_methods,
            },
            virtual_slots,
            interface_slots,
        )
        .expect("the MIR test fixture dispatch slots have persistent identities");
        let public_surface = hir::PublicSemanticSurface::default();
        let export_binding_identities = hir::HirExportBindingIdentities::from_public_surface(
            hir::HirExportBindingIdentityInputs {
                surface: &public_surface,
                structs: &self.structs,
                enums: &self.enums,
                classes: &self.classes,
                interfaces: &self.interfaces,
                objects: &Arena::new(),
                singleton_values: &Arena::new(),
                functions: &self.functions,
                properties: &self.properties,
                type_aliases: &Arena::new(),
                nominal_identities: &nominal_identities,
                object_value_identities: &object_value_identities,
                function_identities: &function_identities,
                property_identities: &property_identities,
                type_alias_identities: &type_alias_identities,
            },
        )
        .expect("the empty MIR test public surface has no export bindings");
        let source_files = vec![hir::SourceFileMetadata {
            identity: scoop_identity::SourceIdentity::single_file(),
            provider: hir::IntrinsicProviderId::from_raw(0),
            name: "<test>".to_string(),
            source: String::new(),
        }];
        let source_context_identities =
            hir::HirSourceContextIdentities::from_contexts(hir::HirSourceContextIdentityInputs {
                source_files: &source_files,
                source_contexts: &source_contexts,
                structs: &self.structs,
                enums: &self.enums,
                classes: &self.classes,
                interfaces: &self.interfaces,
                objects: &Arena::new(),
                functions: &self.functions,
                struct_constructors: &self.struct_constructors,
                class_constructors: &self.class_constructors,
                properties: &self.properties,
                initialization_units: &Arena::new(),
                singleton_values: &Arena::new(),
                lambdas: &Arena::new(),
                anonymous_functions: &Arena::new(),
                nominal_identities: &nominal_identities,
                function_identities: &function_identities,
                property_accessor_identities: &property_accessor_identities,
                constructor_identities: &constructor_identities,
                property_identities: &property_identities,
                initialization_unit_identities: &initialization_unit_identities,
            })
            .expect("the MIR test fixture source contexts have persistent identities");
        let source_native_contracts =
            hir::HirSourceNativeContracts::from_declarations(hir::HirSourceNativeContractInputs {
                functions: &self.functions,
                extern_functions: &self.extern_functions,
                globals: &Arena::new(),
                properties: &self.properties,
                function_identities: &function_identities,
                property_identities: &property_identities,
                type_inputs: type_identity_inputs,
                unit: self.unit,
            })
            .expect("the MIR test fixture externs have source-native contracts");
        let callback_registration_identities =
            hir::HirCallbackRegistrationIdentities::from_registrations(
                hir::HirCallbackRegistrationIdentityInputs {
                    registrations: &Arena::new(),
                    functions: &self.functions,
                    lambdas: &Arena::new(),
                    anonymous_functions: &Arena::new(),
                    local_functions: &Arena::new(),
                    class_constructors: &self.class_constructors,
                    struct_constructors: &self.struct_constructors,
                    function_identities: &function_identities,
                    property_accessor_identities: &property_accessor_identities,
                    constructor_identities: &constructor_identities,
                    enum_member_identities: &enum_member_identities,
                    callback_modes,
                    type_inputs: type_identity_inputs,
                    unit: self.unit,
                },
            )
            .expect("the empty MIR test callback relation is valid");
        let module = hir::Module {
            cone: scoop_identity::ConeIdentity::SINGLE_FILE,
            nominal_identities,
            property_identities,
            property_accessor_identities,
            type_alias_identities,
            enum_member_identities,
            field_identities,
            object_value_identities,
            initialization_unit_identities,
            type_identities,
            constructor_identities,
            function_identities,
            callback_registration_identities,
            export_binding_identities,
            local_binding_identities: hir::HirLocalBindingIdentities::default(),
            dispatch_slot_identities,
            source_context_identities,
            source_native_contracts,
            export_definition_origins,
            public_surface,
            source_files,
            source_contexts,
            types: self.types,
            function_types: self.function_types,
            lambdas: Arena::new(),
            anonymous_functions: Arena::new(),
            local_functions: Arena::new(),
            imported_core_callables: Arena::new(),
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
            intrinsic_type_core,
            source_location_core: hir::SourceLocationCore {
                location: ptr,
                current: entry,
            },
            instantiations: self.instantiations,
        };
        let local_entry = hir::LocalExecutableEntry::try_new(&module, entry)
            .expect("the MIR test harness builds a valid executable entry");
        hir::ExportHirOutput::try_new(
            module,
            hir::ConeOutputKind::Executable {
                local_entry: Box::new(local_entry),
            },
        )
        .expect("the MIR test harness builds a valid executable output")
    }
}
