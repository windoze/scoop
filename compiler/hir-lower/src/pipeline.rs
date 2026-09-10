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

    pub(crate) fn fresh_virtual_method(&mut self, root: hir::FunctionId) -> hir::VirtualMethodId {
        let method = hir::VirtualMethodId::from_raw(self.next_virtual_method_identity);
        self.next_virtual_method_identity += 1;
        let previous = self.virtual_method_roots.insert(method, root);
        assert!(
            previous.is_none(),
            "fresh virtual method identity is unique"
        );
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

    fn alloc_local(
        &mut self,
        name: String,
        ty: TypeId,
        mutable: bool,
        selector: scoop_identity::LocalValueSelector,
    ) -> hir::LocalId {
        let binding = self.fresh_binding();
        self.locals.alloc(hir::Local {
            binding,
            selector,
            name,
            ty,
            mutable,
        })
    }

    pub(crate) fn alloc_this_local(&mut self, ty: TypeId) -> hir::LocalId {
        self.alloc_local(
            "this".to_string(),
            ty,
            false,
            scoop_identity::LocalValueSelector::This,
        )
    }

    pub(crate) fn alloc_parameter_local(
        &mut self,
        name: String,
        ty: TypeId,
        declaration_index: usize,
    ) -> hir::LocalId {
        let declaration_index = u32::try_from(declaration_index)
            .expect("one callable cannot declare more than u32::MAX parameters");
        self.alloc_local(
            name,
            ty,
            false,
            scoop_identity::LocalValueSelector::Parameter { declaration_index },
        )
    }

    pub(crate) fn alloc_declared_local(
        &mut self,
        name: String,
        ty: TypeId,
        mutable: bool,
    ) -> hir::LocalId {
        let path = self
            .definition_paths
            .next(scoop_identity::StructuralDefinitionSiteRole::LocalDeclaration);
        self.alloc_local(
            name,
            ty,
            mutable,
            scoop_identity::LocalValueSelector::LocalDeclaration { path },
        )
    }

    pub(crate) fn alloc_synthetic_local(
        &mut self,
        name: String,
        ty: TypeId,
        mutable: bool,
        role: scoop_identity::SyntheticLocalRole,
    ) -> hir::LocalId {
        let path = self
            .definition_paths
            .next(scoop_identity::StructuralDefinitionSiteRole::SyntheticValue);
        self.alloc_local(
            name,
            ty,
            mutable,
            scoop_identity::LocalValueSelector::Synthetic { path, role },
        )
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
        Lowerer {
            imports: crate::imports::CurrentUnitImports::default(),
            declaration_surface: crate::declaration_surface::DeclarationSurface::default(),
            source_contexts: Arena::new(),
            source_context_by_value: HashMap::new(),
            file_source_contexts: Vec::new(),
            definition_paths: crate::definition_paths::DefinitionPathContext::default(),
            definition_root: None,
            constructor_definition_paths: HashMap::new(),
            types,
            function_types: Arena::new(),
            lambdas: Arena::new(),
            anonymous_functions: Arena::new(),
            next_lambda_function: 0,
            next_anonymous_function: 0,
            next_type_param_identity: 0,
            next_virtual_method_identity: 0,
            virtual_method_roots: HashMap::new(),
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
            source_function_declarations: HashMap::new(),
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
            top_level_namespaces: crate::namespace::TopLevelNamespaces::default(),
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
            extension_receivers: HashMap::new(),
            function_files: HashMap::new(),
            extension_property_by_getter: HashMap::new(),
            property_files: HashMap::new(),
            property_accessor_sources: Vec::new(),
            runtime_accessor_units: HashMap::new(),
            pending_runtime_initializers: Vec::new(),
            current_initialization_unit: None,
            local_delegate_plans: HashMap::new(),
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
            struct_parameter_calling: HashMap::new(),
            class_parameter_calling: HashMap::new(),
            variant_parameter_calling: HashMap::new(),
            signatures: HashMap::new(),
            type_params_in_scope: Vec::new(),
            current_return_ty: unit,
            return_inference: None,
            current_fn_name: String::new(),
            current_source_context: None,
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
        assert!(
            !sources.is_empty(),
            "HIR lowering requires at least one source"
        );
        self.intrinsic_sources = sources;
        self.intrinsic_policy = policy;
        for file in 0..self.intrinsic_sources.len() {
            let context = hir::SourceContext::new(
                self.intrinsic_sources[file].identity.clone(),
                hir::SourceContextSubject::File,
            );
            let id = self.source_contexts.alloc(context.clone());
            assert!(self.source_context_by_value.insert(context, id).is_none());
            self.file_source_contexts.push(id);
        }
        self.current_source_context = self.file_source_contexts.first().copied();
        self
    }

    pub(crate) fn current_intrinsic_provider(&self) -> hir::IntrinsicProviderId {
        self.intrinsic_sources[self.current_file].provider
    }

    pub(crate) fn source_kind(&self, file: usize) -> SourceKind {
        self.intrinsic_sources[file].kind
    }

    pub(crate) fn source_is_core(&self, file: usize) -> bool {
        self.source_kind(file) == SourceKind::ExistingM22Core
    }

    pub(crate) fn current_source_is_core(&self) -> bool {
        self.source_is_core(self.current_file)
    }

    pub(crate) fn sources_are_on_same_side(&self, left: usize, right: usize) -> bool {
        self.top_level_namespaces
            .sources_share_namespace(left, right)
    }

    pub(crate) fn has_top_level_function_candidate(&self, name: &str) -> bool {
        self.top_level_namespaces
            .function_layers(self.current_file, name)
            .iter()
            .any(|layer| !layer.is_empty())
    }

    pub(crate) fn has_top_level_extension_candidate(&self, name: &str) -> bool {
        self.top_level_namespaces
            .extension_layers(self.current_file, name)
            .iter()
            .any(|layer| !layer.is_empty())
    }

    pub(crate) fn has_top_level_property_candidate(&self, name: &str) -> bool {
        self.top_level_namespaces
            .property_layers(self.current_file, name)
            .iter()
            .any(|layer| !layer.is_empty())
    }

    pub(crate) fn primary_user_file(&self) -> usize {
        self.intrinsic_sources
            .iter()
            .enumerate()
            .filter(|(_, source)| source.kind == SourceKind::CurrentUnit)
            .map(|(file, source)| (&source.identity, file))
            .min_by_key(|(identity, _)| *identity)
            .map(|(_, file)| file)
            .expect("a lowering input always contains a current-unit source")
    }

    pub(crate) fn primary_core_file(&self) -> Option<usize> {
        self.intrinsic_sources
            .iter()
            .position(|source| source.kind == SourceKind::ExistingM22Core)
    }

    pub(crate) fn core_diagnostic_file(&self) -> usize {
        self.primary_core_file()
            .unwrap_or_else(|| self.primary_user_file())
    }

    pub(crate) fn current_provider_may_declare_intrinsics(&self) -> bool {
        let source = &self.intrinsic_sources[self.current_file];
        self.current_source_is_core()
            || matches!(
                &self.intrinsic_policy,
                IntrinsicDeclarationPolicy::AllowListedForTesting { providers }
                    if providers.contains(&source.provider)
            )
    }
}
