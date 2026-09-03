use super::*;

impl Lowerer {
    pub(crate) fn run(mut self, files: &[ast::SourceFile]) -> Result<hir::Module, Vec<Diagnostic>> {
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
            let parents = self.resolve_supertype_interface_list(&decl.supertypes);
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
            let interfaces = self.resolve_supertype_interface_list(&decl.supertypes);
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
        self.validate_core_operator_intrinsics(files);
        self.validate_array_conversion_intrinsics(files);
        let source_location_core = self.validate_source_location_core(files);

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
        self.lower_export_parameter_interfaces(
            &pending_functions,
            &pending_methods,
            &pending_structs,
            &pending_classes,
            &pending_enums,
        );
        self.resolve_constructor_graphs(&pending_classes, &pending_structs);
        self.lower_constructor_initialization(&pending_classes, &pending_structs);
        self.declare_derived_equality_methods();
        // Compiler-generated exception edges receive complete typed class /
        // zero-argument-constructor identities after inheritance has been
        // validated and before body lowering. MIR never recovers these
        // targets from names.
        let exception_core = self.validate_exception_core(files);

        // Pass 3: lower bodies. Intrinsics have no body to lower (the
        // parser guarantees it is omitted); their `kind` was set at
        // declaration time. Constructor edges were resolved after source
        // parameter interfaces so delegation shares the ordinary call
        // protocol without depending on body declaration order.
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
        let source_location_core = source_location_core
            .expect("a missing or invalid source location core is always diagnosed");
        Ok(hir::Module {
            source_files: self
                .intrinsic_sources
                .into_iter()
                .map(|source| hir::SourceFileMetadata {
                    provider: source.provider,
                    name: source.name,
                    source: source.source,
                })
                .collect(),
            source_contexts: self.source_contexts,
            types: self.types,
            function_types: self.function_types,
            lambdas: self.lambdas,
            anonymous_functions: self.anonymous_functions,
            local_functions: self.local_functions,
            callable_references: self.callable_references,
            bound_callable_refs: self.bound_callable_refs,
            function_coercions: self.function_coercions,
            foreign_callback_registrations: self.foreign_callback_registrations,
            source_parameter_interfaces: self.source_parameter_interfaces,
            export_default_exprs: self.export_default_exprs,
            export_default_sources: self.export_default_sources,
            export_vararg_parameter_types: self.export_vararg_parameter_types,
            functions: self.functions,
            extern_functions: self.extern_functions,
            globals: self.globals,
            generic_functions: self.generic_functions,
            method_applications: self.method_applications,
            generic_methods: self.generic_methods,
            generic_method_applications: self.generic_method_applications,
            derived_equality_applications: self.derived_equality_applications,
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
            source_location_core,
            entry,
            instantiations: self.instantiations,
        })
    }
}
