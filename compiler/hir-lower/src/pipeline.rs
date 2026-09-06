use super::*;
mod run;

impl Lowerer {
    pub(crate) fn fresh_type_param(&mut self, substitution_slot: usize) -> hir::TypeParamId {
        let parameter = hir::TypeParamId::with_substitution_slot(
            self.next_type_param_identity,
            substitution_slot as u32,
        );
        self.next_type_param_identity += 1;
        parameter
    }

    pub(crate) fn fresh_virtual_method(&mut self) -> hir::VirtualMethodId {
        let method = hir::VirtualMethodId::from_raw(self.next_virtual_method_identity);
        self.next_virtual_method_identity += 1;
        method
    }

    pub(crate) fn fresh_constructor_parameter(&mut self) -> hir::ConstructorParamId {
        let parameter = hir::ConstructorParamId::from_raw(self.next_constructor_parameter_identity);
        self.next_constructor_parameter_identity += 1;
        let binding = self.fresh_binding();
        assert!(
            self.constructor_parameter_bindings
                .insert(parameter, binding)
                .is_none(),
            "constructor parameter identities are globally unique"
        );
        parameter
    }

    pub(crate) fn fresh_binding(&mut self) -> hir::BindingId {
        let binding = hir::BindingId::from_raw(self.next_binding_id);
        self.next_binding_id += 1;
        binding
    }

    pub(crate) fn fresh_loop(&mut self) -> hir::LoopId {
        let identity = self.next_loop_identity;
        self.next_loop_identity = identity
            .checked_add(1)
            .expect("Export HIR loop identity space exhausted");
        hir::LoopId::from_raw(identity)
    }

    pub(crate) fn alloc_local(&mut self, name: String, ty: TypeId, mutable: bool) -> hir::LocalId {
        let binding = self.fresh_binding();
        self.locals.alloc(hir::Local {
            binding,
            name,
            ty,
            mutable,
        })
    }

    pub(crate) fn push_scope(&mut self) {
        self.scopes.push();
        self.local_function_scopes.push();
    }

    pub(crate) fn pop_scope(&mut self) {
        self.scopes.pop();
        self.local_function_scopes.pop();
    }

    pub(crate) fn integer_type(&self, kind: hir::IntegerKind) -> TypeId {
        self.integer_types.owner(kind)
    }

    pub(super) fn new() -> Self {
        // Well-known types are allocated first, in a fixed order (impl spec
        // 2.2): Unit, the eight canonical integers in `IntegerKind::ALL`
        // order, Boolean and String. `Any` follows them and remains an
        // internal lowering identity.
        let mut types = Arena::new();
        let unit = types.alloc(Type::Unit);
        let integer_types = hir::IntegerTypeCore::new(
            hir::IntegerKind::ALL.map(|kind| types.alloc(Type::Integer(kind))),
        )
        .expect("fresh integer type ids are distinct");
        let boolean = types.alloc(Type::Boolean);
        let string = types.alloc(Type::String);
        let any = types.alloc(Type::Any);
        let mut source_contexts = Arena::new();
        let root_source_context = source_contexts.alloc(hir::SourceContext {
            function_name: String::new(),
            type_name: String::new(),
        });

        Lowerer {
            source_contexts,
            types,
            function_types: Arena::new(),
            lambdas: Arena::new(),
            anonymous_functions: Arena::new(),
            next_lambda_function: 0,
            next_anonymous_function: 0,
            next_type_param_identity: 0,
            next_virtual_method_identity: 0,
            next_constructor_parameter_identity: 0,
            next_loop_identity: 0,
            local_functions: Arena::new(),
            local_function_by_function: HashMap::new(),
            callable_references: Arena::new(),
            bound_callable_refs: Arena::new(),
            function_coercions: Arena::new(),
            foreign_callback_registrations: Arena::new(),
            source_parameter_interfaces: Vec::new(),
            export_default_exprs: Arena::new(),
            export_default_sources: Arena::new(),
            export_vararg_parameter_types: Arena::new(),
            local_default_exprs: Arena::new(),
            default_templates: HashMap::new(),
            lowering_default_template: false,
            function_coercion_by_types: HashMap::new(),
            structs: Arena::new(),
            struct_constructors: Arena::new(),
            struct_constructor_applications: Arena::new(),
            struct_constructor_application_by_key: HashMap::new(),
            struct_applications: Arena::new(),
            struct_application_by_key: HashMap::new(),
            enums: Arena::new(),
            enum_applications: Arena::new(),
            enum_application_by_key: HashMap::new(),
            classes: Arena::new(),
            class_fields: Arena::new(),
            class_constructors: Arena::new(),
            class_constructor_applications: Arena::new(),
            class_constructor_application_by_key: HashMap::new(),
            class_applications: Arena::new(),
            class_application_by_key: HashMap::new(),
            interfaces: Arena::new(),
            interface_applications: Arena::new(),
            interface_application_by_key: HashMap::new(),
            functions: Arena::new(),
            intrinsic_functions: HashMap::new(),
            intrinsic_type_owners: HashMap::new(),
            extern_functions: Arena::new(),
            globals: Arena::new(),
            initialization_units: Arena::new(),
            initialization_failure_roots: Arena::new(),
            objects: Arena::new(),
            object_types: Arena::new(),
            companion_relations: Arena::new(),
            companion_by_host: HashMap::new(),
            singleton_values: Arena::new(),
            singleton_published_roots: Arena::new(),
            properties: Arena::new(),
            extension_properties: Arena::new(),
            property_getters: Arena::new(),
            property_setters: Arena::new(),
            delegate_storages: Arena::new(),
            source_type_aliases: Arena::new(),
            source_type_aliases_by_name: HashMap::new(),
            type_aliases: Arena::new(),
            type_alias_resolution_stack: Vec::new(),
            generic_functions: Arena::new(),
            method_applications: Arena::new(),
            method_application_by_key: HashMap::new(),
            generic_methods: Arena::new(),
            generic_method_applications: Arena::new(),
            generic_method_application_by_key: HashMap::new(),
            derived_equality_applications: Arena::new(),
            derived_equality_application_by_type: HashMap::new(),
            top_level: Vec::new(),
            unit,
            integer_types,
            boolean,
            string,
            any,
            functions_by_name: HashMap::new(),
            extensions_by_name: HashMap::new(),
            extension_receivers: HashMap::new(),
            function_files: HashMap::new(),
            properties_by_name: HashMap::new(),
            extension_properties_by_name: HashMap::new(),
            extension_property_by_getter: HashMap::new(),
            property_files: HashMap::new(),
            property_accessor_sources: Vec::new(),
            runtime_accessor_units: HashMap::new(),
            pending_runtime_initializers: Vec::new(),
            current_initialization_unit: None,
            local_delegate_plans: HashMap::new(),
            user_file_index: 0,
            structs_by_name: HashMap::new(),
            enums_by_name: HashMap::new(),
            classes_by_name: HashMap::new(),
            interfaces_by_name: HashMap::new(),
            objects_by_name: HashMap::new(),
            object_by_backing_class: HashMap::new(),
            nested_nominals_by_owner: HashMap::new(),
            struct_files: HashMap::new(),
            enum_files: HashMap::new(),
            class_files: HashMap::new(),
            interface_files: HashMap::new(),
            object_files: HashMap::new(),
            ffi_ptr: None,
            ffi_fun_ptr: None,
            ffi_pinned_ptr: None,
            ffi_gc_handle: None,
            ffi_foreign_callback: None,
            ffi_core: None,
            foreign_callback_core: None,
            allow_deferred_fun_ptr: false,
            pointer_type_uses: Vec::new(),
            fun_ptr_type_uses: Vec::new(),
            interface_methods: HashMap::new(),
            interface_method_entities: Arena::new(),
            function_owner: HashMap::new(),
            override_sources: HashMap::new(),
            override_default_type_arguments: HashMap::new(),
            option_candidates: Vec::new(),
            pending_option_enum: None,
            option_core: None,
            iteration_core: None,
            core_prelude_variants: CorePreludeVariantBindings::default(),
            throwable_candidates: Vec::new(),
            throwable: None,
            variant_styles: HashMap::new(),
            struct_parameter_calling: HashMap::new(),
            class_parameter_calling: HashMap::new(),
            variant_parameter_calling: HashMap::new(),
            signatures: HashMap::new(),
            type_params_in_scope: Vec::new(),
            current_return_ty: unit,
            return_inference: None,
            current_fn_name: String::new(),
            current_source_context: root_source_context,
            suspension_contexts: vec![SuspensionContext::Forbidden(
                ForbiddenSuspendContext::TopLevel,
            )],
            safety_contexts: vec![hir::Safety::Safe],
            current_this: None,
            current_owner: None,
            constructor_params_in_scope: HashMap::new(),
            constructor_parameter_bindings: HashMap::new(),
            initialization_context: None,
            backing_field_context: None,
            smart_casts: HashMap::new(),
            current_file: 0,
            intrinsic_sources: Vec::new(),
            intrinsic_policy: IntrinsicDeclarationPolicy::CoreOnly,
            locals: Arena::new(),
            scopes: Scopes::new(),
            local_function_scopes: LocalFunctionScopes::new(),
            loop_targets: Vec::new(),
            capture_contexts: Vec::new(),
            next_binding_id: 0,
            instantiations: Arena::new(),
            hidden_count: 0,
            diagnostics: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub(super) fn with_intrinsic_sources(
        mut self,
        sources: Vec<SourceProvider>,
        policy: IntrinsicDeclarationPolicy,
    ) -> Self {
        self.intrinsic_sources = sources;
        self.intrinsic_policy = policy;
        self
    }

    pub(crate) fn current_intrinsic_provider(&self) -> hir::IntrinsicProviderId {
        self.intrinsic_sources[self.current_file].provider
    }

    pub(crate) fn current_provider_may_declare_intrinsics(&self) -> bool {
        let source = &self.intrinsic_sources[self.current_file];
        source.core
            || matches!(
                &self.intrinsic_policy,
                IntrinsicDeclarationPolicy::AllowListedForTesting { providers }
                    if providers.contains(&source.provider)
            )
    }
}
