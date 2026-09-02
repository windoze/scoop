use super::*;

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

    pub(crate) fn fresh_binding(&mut self) -> hir::BindingId {
        let binding = hir::BindingId::from_raw(self.next_binding_id);
        self.next_binding_id += 1;
        binding
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

    pub(super) fn new() -> Self {
        // Well-known types are allocated first, in a fixed order
        // (impl spec 2.2): Unit, Int, UInt (M9), Boolean, String.
        // `Any` (M6) follows them; it is not part of the `hir::Module`
        // well-known list, so hir-lower interns it once here.
        let mut types = Arena::new();
        let unit = types.alloc(Type::Unit);
        let int = types.alloc(Type::Int);
        let uint = types.alloc(Type::UInt);
        let boolean = types.alloc(Type::Boolean);
        let string = types.alloc(Type::String);
        let any = types.alloc(Type::Any);

        Lowerer {
            types,
            function_types: Arena::new(),
            lambdas: Arena::new(),
            anonymous_functions: Arena::new(),
            next_lambda_function: 0,
            next_anonymous_function: 0,
            next_type_param_identity: 0,
            next_virtual_method_identity: 0,
            local_functions: Arena::new(),
            local_function_by_function: HashMap::new(),
            callable_references: Arena::new(),
            bound_callable_refs: Arena::new(),
            function_coercions: Arena::new(),
            foreign_callback_registrations: Arena::new(),
            function_coercion_by_types: HashMap::new(),
            structs: Arena::new(),
            struct_applications: Arena::new(),
            struct_application_by_key: HashMap::new(),
            enums: Arena::new(),
            enum_applications: Arena::new(),
            enum_application_by_key: HashMap::new(),
            classes: Arena::new(),
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
            int,
            uint,
            boolean,
            string,
            any,
            functions_by_name: HashMap::new(),
            extensions_by_name: HashMap::new(),
            extension_receivers: HashMap::new(),
            function_files: HashMap::new(),
            globals_by_name: HashMap::new(),
            global_files: HashMap::new(),
            user_file_index: 0,
            structs_by_name: HashMap::new(),
            enums_by_name: HashMap::new(),
            classes_by_name: HashMap::new(),
            interfaces_by_name: HashMap::new(),
            struct_files: HashMap::new(),
            enum_files: HashMap::new(),
            class_files: HashMap::new(),
            interface_files: HashMap::new(),
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
            option_candidates: Vec::new(),
            option_enum: None,
            throwable_candidates: Vec::new(),
            throwable: None,
            variant_styles: HashMap::new(),
            signatures: HashMap::new(),
            type_params_in_scope: Vec::new(),
            current_return_ty: unit,
            return_inference: None,
            current_fn_name: String::new(),
            suspension_contexts: vec![SuspensionContext::Forbidden(
                ForbiddenSuspendContext::TopLevel,
            )],
            safety_contexts: vec![hir::Safety::Safe],
            current_this: None,
            current_owner: None,
            constructor_params_in_scope: HashMap::new(),
            smart_casts: HashMap::new(),
            current_file: 0,
            intrinsic_sources: Vec::new(),
            intrinsic_policy: IntrinsicDeclarationPolicy::CoreOnly,
            locals: Arena::new(),
            scopes: Scopes::new(),
            local_function_scopes: LocalFunctionScopes::new(),
            capture_contexts: Vec::new(),
            next_binding_id: 0,
            instantiations: Arena::new(),
            hidden_count: 0,
            diagnostics: Vec::new(),
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
        let source = self.intrinsic_sources[self.current_file];
        source.core
            || matches!(
                &self.intrinsic_policy,
                IntrinsicDeclarationPolicy::AllowListedForTesting { providers }
                    if providers.contains(&source.provider)
            )
    }

    pub(super) fn run(mut self, files: &[ast::SourceFile]) -> Result<hir::Module, Vec<Diagnostic>> {
        if files.is_empty() {
            return Err(vec![Diagnostic {
                file: 0,
                span: None,
                message: "no source files to compile".to_string(),
            }]);
        }
        let user_file_index = files.len() - 1;
        self.user_file_index = user_file_index;

        // Pass 1: declare structs, enums, classes, interfaces and
        // functions across all files (core first), so bodies and field
        // types resolve regardless of declaration order. Structs,
        // enums, classes and interfaces share the *type* namespace and
        // must not collide; functions occupy a separate namespace where
        // one name may collect several overloads (M7), and member
        // functions live in per-owner namespaces.
        let mut pending_structs = Vec::new();
        let mut pending_enums = Vec::new();
        let mut pending_classes = Vec::new();
        let mut pending_interfaces = Vec::new();
        let mut pending_functions = Vec::new();
        let mut pending_globals = Vec::new();
        let mut pending_methods: Vec<(FunctionId, &ast::FunctionDecl, usize, Owner)> = Vec::new();
        for (file_index, file) in files.iter().enumerate() {
            self.current_file = file_index;
            let is_core = self.intrinsic_sources[file_index].core;
            for decl in &file.declarations {
                match decl {
                    ast::Decl::Global(decl) => pending_globals.push((decl, file_index)),
                    ast::Decl::Struct(decl) => self.declare_struct(
                        decl,
                        &mut pending_structs,
                        &mut pending_methods,
                        file_index,
                    ),
                    ast::Decl::Enum(decl) => self.declare_enum(
                        decl,
                        is_core,
                        &mut pending_enums,
                        &mut pending_methods,
                        file_index,
                    ),
                    ast::Decl::Class(decl) => self.declare_class(
                        decl,
                        is_core,
                        &mut pending_classes,
                        &mut pending_methods,
                        file_index,
                    ),
                    ast::Decl::Interface(decl) => self.declare_interface(
                        decl,
                        &mut pending_interfaces,
                        &mut pending_methods,
                        file_index,
                    ),
                    ast::Decl::Function(decl) => {
                        self.declare_function(decl, &mut pending_functions, file_index)
                    }
                }
            }
        }

        // Type-parameter names and arities are declared in pass 1. Resolve
        // their ordered constraints only after every nominal name is visible,
        // then validate bound applications after all constraint sets are
        // complete (F-bounds may form legal dependency cycles).
        for &(id, decl, file_index) in &pending_structs {
            self.current_file = file_index;
            let declared = self.structs[id].type_params.clone();
            let params = self.resolve_type_parameter_constraints(
                declared,
                0,
                &decl.type_params,
                decl.where_clause.as_ref(),
                "struct",
            );
            self.structs[id].type_params = params;
        }
        for &(id, decl, file_index) in &pending_enums {
            self.current_file = file_index;
            let declared = self.enums[id].type_params.clone();
            let params = self.resolve_type_parameter_constraints(
                declared,
                0,
                &decl.type_params,
                decl.where_clause.as_ref(),
                "enum",
            );
            self.enums[id].type_params = params;
        }
        for &(id, decl, file_index) in &pending_classes {
            self.current_file = file_index;
            let declared = self.classes[id].type_params.clone();
            let params = self.resolve_type_parameter_constraints(
                declared,
                0,
                &decl.type_params,
                decl.where_clause.as_ref(),
                "class",
            );
            self.classes[id].type_params = params;
        }
        for &(id, decl, file_index) in &pending_interfaces {
            self.current_file = file_index;
            let declared = self.interfaces[id].type_params.clone();
            let params = self.resolve_type_parameter_constraints(
                declared,
                0,
                &decl.type_params,
                decl.where_clause.as_ref(),
                "interface",
            );
            self.interfaces[id].type_params = params;
        }
        self.validate_nominal_type_parameter_constraints();
        let intrinsic_type_core = self.validate_intrinsic_type_core(files);
        for &(id, decl, file_index) in &pending_interfaces {
            self.current_file = file_index;
            self.type_params_in_scope = self.interfaces[id].type_params.clone();
            let parents = self.resolve_interface_list(&decl.parents);
            self.type_params_in_scope.clear();
            self.interfaces[id].parents = parents
                .into_iter()
                .map(|parent| match self.types[parent] {
                    Type::Interface(application) => application,
                    _ => unreachable!("resolved interface parents are interface applications"),
                })
                .collect();
        }
        self.check_interface_inheritance_cycles(&pending_interfaces);

        self.ffi_ptr = self.require_core_struct("Ptr", files);
        self.ffi_fun_ptr = self.require_core_struct("FunPtr", files);
        self.ffi_pinned_ptr = self.require_core_struct("PinnedPtr", files);
        self.ffi_gc_handle = self.require_core_struct("GcHandle", files);
        self.ffi_foreign_callback = self.require_core_struct("ForeignCallback", files);

        // The core library's `Option<T>` must be validated before any
        // type annotation is resolved: `T?` desugars to it (spec 7.1).
        self.validate_option_enum(files);
        // The core library's `Throwable` is the root every `throw`
        // operand and catch parameter type is checked against (spec
        // 11.7).
        self.validate_throwable(files);

        // Pass 2: resolve struct fields, enum variants, class
        // constructor properties and inheritance clauses (all type
        // names are known now, so fields may reference later-declared
        // types).
        for &(id, decl, file_index) in &pending_structs {
            self.current_file = file_index;
            self.allow_deferred_fun_ptr = Some(id) == self.ffi_foreign_callback;
            self.resolve_fields(id, decl);
            self.allow_deferred_fun_ptr = false;
            self.type_params_in_scope = self.structs[id].type_params.clone();
            let interfaces = self.resolve_interface_list(&decl.interfaces);
            self.type_params_in_scope.clear();
            self.structs[id].interfaces = interfaces;
        }
        for &(id, decl, file_index) in &pending_enums {
            self.current_file = file_index;
            self.resolve_variants(id, decl);
            self.type_params_in_scope = self.enums[id].type_params.clone();
            let interfaces = self.resolve_interface_list(&decl.interfaces);
            self.type_params_in_scope.clear();
            self.enums[id].interfaces = interfaces;
        }
        for (id, decl, file_index) in &pending_classes {
            self.current_file = *file_index;
            self.resolve_class(*id, decl);
        }

        // Pass 2.5: resolve function and method signatures, so calls
        // in any body see parameter and return types regardless of
        // declaration order. Interface method signatures become
        // `hir::MethodSig`s; bodyless declarations (interface and
        // abstract methods) get their parameter-only body here.
        for &(id, decl, file_index) in &pending_functions {
            self.current_file = file_index;
            self.resolve_signature(id, decl);
        }
        for &(id, decl, file_index, owner) in &pending_methods {
            self.current_file = file_index;
            self.resolve_method_signature(id, decl, owner);
        }

        // Declaration-site variance is a property of the fully resolved
        // interface signatures, so validate it after every signature exists.
        self.check_interface_variance();

        // M10's coroutine protocol is compiler-known: MIR generation needs
        // these exact generic interfaces and intrinsic signatures rather than
        // guessing entities from names after HIR.
        let coroutine_core = self.validate_coroutine_core(files);
        let ffi_core = self.validate_ffi_core(files);
        self.ffi_core = ffi_core;
        let foreign_callback_core = self.validate_foreign_callback_core(files);
        self.foreign_callback_core = foreign_callback_core;
        self.validate_pointer_type_uses();
        self.resolve_globals(&pending_globals);
        self.validate_extern_functions();
        self.validate_extern_global_symbols();

        // Pass 2.6: overload declarations must be distinguishable —
        // within one name (top-level) or one host (members) no two
        // functions may share a signature (milestone7 DESIGN.md 1.1).
        self.check_duplicate_signatures(&pending_functions, &pending_methods);

        // Pass 2.75: inheritance checks (milestone6 DESIGN.md 2.2) —
        // cycles, property shadowing, override rules and interface
        // implementation (classes and value types alike). Needs every
        // signature and inheritance clause.
        self.check_inheritance(
            &pending_classes,
            &pending_structs,
            &pending_enums,
            &pending_methods,
        );
        self.declare_derived_equality_methods();
        // Compiler-generated exception edges receive complete typed class /
        // zero-argument-constructor identities after inheritance has been
        // validated and before body lowering. MIR never recovers these
        // targets from names.
        let exception_core = self.validate_exception_core(files);

        // Pass 3: lower bodies. Intrinsics have no body to lower (the
        // parser guarantees it is omitted); their `kind` was set at
        // declaration time. Base-constructor delegation arguments are
        // lowered in an empty scope (constructor properties are not in
        // scope there, an M6 simplification: HIR has no body to host
        // their locals).
        for &(id, decl, file_index) in &pending_classes {
            self.current_file = file_index;
            self.lower_base_args(id, decl);
        }
        for (id, decl, file_index) in pending_functions {
            if matches!(
                self.functions[id].kind,
                FunctionKind::Intrinsic(_) | FunctionKind::Extern(_)
            ) {
                continue;
            }
            self.current_file = file_index;
            let body = self.lower_body(id, decl);
            self.functions[id].kind = FunctionKind::User(body);
        }
        for (id, decl, file_index, owner) in pending_methods {
            // Interface and abstract methods are bodyless; their
            // parameter-only body was built in pass 2.5.
            if matches!(self.functions[id].kind, FunctionKind::Intrinsic(_))
                || decl.modifier == ast::MethodModifier::Abstract
                || matches!(owner, Owner::Interface(_))
            {
                continue;
            }
            self.current_file = file_index;
            let body = self.lower_body(id, decl);
            self.functions[id].kind = FunctionKind::User(body);
        }

        // Effects consume fully resolved calls and types. Local functions and
        // callable literals lifted while lowering the bodies are visible now.
        self.validate_c_ffi_types();
        self.check_generic_recursion();
        self.check_no_gc_types();
        self.check_no_gc_functions();

        // A module without `main` never reaches HIR (hir docs); it is a
        // diagnostic here, attributed to the user file. With overloads
        // (M7) several functions may be named `main`; the entry point
        // is the zero-parameter one.
        self.current_file = user_file_index;
        let zero_param_main = self.functions_by_name.get("main").and_then(|ids| {
            ids.iter().copied().find(|&id| {
                self.signatures
                    .get(&id)
                    .is_some_and(|sig| sig.params.is_empty())
            })
        });
        let entry = match zero_param_main {
            Some(id) => {
                // The entry point is monomorphic: there is no caller to
                // infer type arguments from.
                if !self.functions[id].type_params().is_empty() {
                    self.error(
                        self.functions[id].span,
                        "`main` must not be generic".to_string(),
                    );
                }
                if self.functions[id].is_suspend {
                    self.error(
                        self.functions[id].span,
                        "`main` must not be suspend".to_string(),
                    );
                }
                if matches!(self.functions[id].kind, FunctionKind::Extern(_)) {
                    self.error(
                        self.functions[id].span,
                        "`main` must be a Scoop-defined function".to_string(),
                    );
                }
                Some(id)
            }
            None => {
                self.error(
                    files[user_file_index].span,
                    "missing entry point: declare `fun main()`".to_string(),
                );
                None
            }
        };

        if !self.diagnostics.is_empty() {
            return Err(self.diagnostics);
        }
        // Invariant: empty diagnostics implies `main` was found and the
        // core `Option<T>` validated above.
        let entry = entry.expect("missing `main` is always diagnosed");
        let option_enum = self
            .option_enum
            .expect("a missing or invalid core `Option` is always diagnosed");
        let coroutine_core = coroutine_core
            .expect("a missing or invalid coroutine core protocol is always diagnosed");
        let exception_core = exception_core
            .expect("a missing or invalid compiler exception core is always diagnosed");
        let ffi_core =
            ffi_core.expect("a missing or invalid FFI core protocol is always diagnosed");
        Ok(hir::Module {
            types: self.types,
            function_types: self.function_types,
            lambdas: self.lambdas,
            anonymous_functions: self.anonymous_functions,
            local_functions: self.local_functions,
            callable_references: self.callable_references,
            bound_callable_refs: self.bound_callable_refs,
            function_coercions: self.function_coercions,
            foreign_callback_registrations: self.foreign_callback_registrations,
            functions: self.functions,
            extern_functions: self.extern_functions,
            globals: self.globals,
            generic_functions: self.generic_functions,
            method_applications: self.method_applications,
            generic_methods: self.generic_methods,
            generic_method_applications: self.generic_method_applications,
            derived_equality_applications: self.derived_equality_applications,
            structs: self.structs,
            struct_applications: self.struct_applications,
            enums: self.enums,
            enum_applications: self.enum_applications,
            classes: self.classes,
            class_applications: self.class_applications,
            interfaces: self.interfaces,
            interface_applications: self.interface_applications,
            interface_methods: self.interface_method_entities,
            top_level: self.top_level,
            unit: self.unit,
            int: self.int,
            boolean: self.boolean,
            string: self.string,
            option_enum,
            exception_core,
            coroutine_core,
            ffi_core,
            foreign_callback_core: foreign_callback_core
                .expect("a missing or invalid foreign callback core protocol is always diagnosed"),
            intrinsic_type_core: intrinsic_type_core
                .expect("missing or invalid intrinsic core types are always diagnosed"),
            entry,
            instantiations: self.instantiations,
        })
    }
}
