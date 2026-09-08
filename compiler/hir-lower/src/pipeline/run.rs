use super::*;

impl Lowerer {
    pub(crate) fn run(
        mut self,
        files: &[ast::SourceFile],
    ) -> Result<(hir::Module, Vec<Diagnostic>), Vec<Diagnostic>> {
        if files.is_empty() {
            return Err(vec![Diagnostic {
                severity: ast::DiagnosticSeverity::Error,
                file: 0,
                span: None,
                message: "no source files to compile".to_string(),
            }]);
        }
        let user_file_index = files.len() - 1;
        self.user_file_index = user_file_index;

        // Pass 1: declare aliases, nominals and functions across all files
        // (core first), so bodies and field types resolve regardless of
        // declaration order. Aliases and top-level nominals share the type
        // namespace and must not collide; functions occupy a separate
        // namespace where one name may collect several overloads (M7), and
        // member functions live in per-owner namespaces.
        let mut pending_structs = Vec::new();
        let mut pending_enums = Vec::new();
        let mut pending_classes = Vec::new();
        // File packages and imports are collected structurally before any
        // name resolution; packages intern by segment equality so files
        // declaring the same package share one id.
        for file in files {
            let package = match &file.package {
                ast::PackageSyntax::RootPackage => hir::PackageDecl::root(),
                ast::PackageSyntax::QualifiedPackage(path) => hir::PackageDecl {
                    segments: path
                        .segments
                        .iter()
                        .map(|segment| segment.text.clone())
                        .collect(),
                },
            };
            let package_id = match self
                .package_decls
                .iter()
                .position(|existing| *existing == package)
            {
                Some(index) => hir::PackageId::from_raw(la_arena::RawIdx::from_u32(index as u32)),
                None => {
                    let index = self.package_decls.len();
                    self.package_decls.push(package);
                    hir::PackageId::from_raw(la_arena::RawIdx::from_u32(index as u32))
                }
            };
            let mut exact = Vec::new();
            let mut star = Vec::new();
            for import in &file.imports {
                match import {
                    ast::ImportSyntax::Exact {
                        public,
                        path,
                        alias,
                        span,
                    } => exact.push(hir::ExactImport {
                        public: *public,
                        path: path
                            .segments
                            .iter()
                            .map(|segment| segment.text.clone())
                            .collect(),
                        alias: alias.as_ref().map(|alias| alias.text.clone()),
                        span: *span,
                        binding: None,
                    }),
                    ast::ImportSyntax::Star { public, path, span } => {
                        star.push(hir::StarImport {
                            public: *public,
                            path: path
                                .segments
                                .iter()
                                .map(|segment| segment.text.clone())
                                .collect(),
                            span: *span,
                            binding: None,
                        });
                    }
                }
            }
            self.file_packages.push(package_id);
            self.file_imports.push(hir::FileImports { exact, star });
            self.file_star_variants.push(HashMap::new());
        }

        let mut pending_interfaces = Vec::new();
        let mut pending_objects = Vec::new();
        let mut pending_functions = Vec::new();
        let mut pending_globals = Vec::new();
        let mut pending_methods: Vec<(FunctionId, &ast::FunctionDecl, usize, Owner)> = Vec::new();
        for (file_index, file) in files.iter().enumerate() {
            self.current_file = file_index;
            let is_core = self.intrinsic_sources[file_index].core;
            for decl in &file.declarations {
                match decl {
                    ast::Decl::Global(decl) => pending_globals.push((decl, file_index)),
                    ast::Decl::Struct(decl) => {
                        let _ = self.declare_struct(
                            decl,
                            &mut pending_structs,
                            &mut pending_methods,
                            file_index,
                            None,
                        );
                    }
                    ast::Decl::Enum(decl) => {
                        let _ = self.declare_enum(
                            decl,
                            is_core,
                            &mut pending_enums,
                            &mut pending_methods,
                            file_index,
                            None,
                        );
                    }
                    ast::Decl::Class(decl) => {
                        let _ = self.declare_class(
                            decl,
                            is_core,
                            &mut pending_classes,
                            &mut pending_methods,
                            file_index,
                            None,
                        );
                    }
                    ast::Decl::Interface(decl) => {
                        let _ = self.declare_interface(
                            decl,
                            &mut pending_interfaces,
                            &mut pending_methods,
                            file_index,
                            None,
                        );
                    }
                    ast::Decl::Object(decl) => {
                        let _ = self.declare_object(
                            decl,
                            &mut pending_objects,
                            &mut pending_methods,
                            file_index,
                            None,
                        );
                    }
                    ast::Decl::TypeAlias(decl) => {
                        self.declare_type_alias(decl, file_index);
                    }
                    ast::Decl::Function(decl) => {
                        self.declare_function(decl, &mut pending_functions, file_index)
                    }
                }
            }
        }

        // Static nested declarations share the same semantic passes as
        // top-level nominals, but live in an owner-scoped typed namespace.
        // Declare the complete tree before resolving any signature so
        // sibling and forward-qualified references are order-independent.
        let root_structs = pending_structs.clone();
        let root_enums = pending_enums.clone();
        let root_classes = pending_classes.clone();
        let root_interfaces = pending_interfaces.clone();
        let root_objects = pending_objects.clone();
        let mut nested_queues = crate::declarations::NestedDeclarationQueues {
            structs: &mut pending_structs,
            enums: &mut pending_enums,
            classes: &mut pending_classes,
            interfaces: &mut pending_interfaces,
            objects: &mut pending_objects,
            methods: &mut pending_methods,
        };
        for (id, declaration, file) in root_structs {
            self.declare_struct_nested(Owner::Struct(id), declaration, &mut nested_queues, file);
        }
        for (id, declaration, file) in root_enums {
            self.declare_enum_nested(Owner::Enum(id), declaration, &mut nested_queues, file);
        }
        for (id, declaration, file) in root_classes {
            self.declare_class_nested(Owner::Class(id), declaration, &mut nested_queues, file);
        }
        for (id, declaration, file) in root_interfaces {
            self.declare_interface_nested(
                Owner::Interface(id),
                declaration,
                &mut nested_queues,
                file,
            );
        }
        for (id, declaration, file) in root_objects {
            self.declare_object_nested(Owner::Object(id), declaration, &mut nested_queues, file);
        }

        // Alias targets may mention any declaration in the Cone, including a
        // later alias, `Option<T>` through nullable syntax, and the special
        // pointer families. Establish those source identities before the
        // alias graph is expanded. Application bounds are checked again once
        // every nominal constraint is complete below.
        self.ffi_ptr = self.require_core_struct("Ptr", files);
        self.ffi_fun_ptr = self.require_core_struct("FunPtr", files);
        self.ffi_pinned_ptr = self.require_core_struct("PinnedPtr", files);
        self.ffi_gc_handle = self.require_core_struct("GcHandle", files);
        self.ffi_foreign_callback = self.require_core_struct("ForeignCallback", files);
        self.validate_option_enum(files);
        self.resolve_all_type_aliases();

        // Type-parameter names and arities are declared in pass 1. Resolve
        // their ordered constraints only after every nominal name is visible,
        // then validate bound applications after all constraint sets are
        // complete (F-bounds may form legal dependency cycles).
        for &(id, decl, file_index) in &pending_structs {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Struct(id));
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
            self.current_owner = Some(Owner::Enum(id));
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
            self.current_owner = Some(Owner::Class(id));
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
            self.current_owner = Some(Owner::Interface(id));
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
        self.current_owner = None;
        self.validate_nominal_type_parameter_constraints();
        let intrinsic_type_core = self.validate_intrinsic_type_core(files);
        if let Some(core) = intrinsic_type_core {
            if self.ffi_ptr != Some(core.ptr) {
                self.current_file = self.user_file_index.min(files.len() - 1);
                self.error(
                    files[0].span,
                    "the `Ptr` FFI core owner must be the `core_ptr` intrinsic type".to_string(),
                );
            }
            if self.ffi_fun_ptr != Some(core.fun_ptr) {
                self.current_file = self.user_file_index.min(files.len() - 1);
                self.error(
                    files[0].span,
                    "the `FunPtr` FFI core owner must be the `core_fun_ptr` intrinsic type"
                        .to_string(),
                );
            }
        }
        for &(id, decl, file_index) in &pending_interfaces {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Interface(id));
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
        self.current_owner = None;
        self.check_interface_inheritance_cycles(&pending_interfaces);

        // The core library's `Option<T>` must be validated before any
        // type annotation is resolved: `T?` desugars to it (spec 7.1).
        // It was validated before alias expansion above because an alias
        // target may itself contain nullable syntax.
        // The core library's `Throwable` is the root every `throw`
        // operand and catch parameter type is checked against (spec
        // 11.7).
        self.validate_throwable(files);

        for &(id, decl, file_index) in &pending_interfaces {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Interface(id));
            self.resolve_interface_properties(id, decl);
        }
        self.current_owner = None;

        // Pass 2: resolve struct fields, enum variants, class
        // constructor properties and inheritance clauses (all type
        // names are known now, so fields may reference later-declared
        // types).
        for &(id, decl, file_index) in &pending_structs {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Struct(id));
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
            self.current_owner = Some(Owner::Enum(id));
            self.resolve_variants(id, decl);
            self.type_params_in_scope = self.enums[id].type_params.clone();
            let interfaces = self.resolve_interface_list(&decl.interfaces);
            self.type_params_in_scope.clear();
            self.enums[id].interfaces = interfaces;
        }
        self.validate_option_variants();
        for (id, decl, file_index) in &pending_classes {
            self.current_file = *file_index;
            self.current_owner = Some(Owner::Class(*id));
            self.resolve_class(*id, decl);
        }
        for &(id, decl, file_index) in &pending_objects {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Object(id));
            self.resolve_object(id, decl);
        }
        self.current_owner = None;

        // Inline value layout must be finite before signatures or bodies can
        // request concrete applications. The declaration graph deliberately
        // stops at every reference/pointer boundary and recognizes generic
        // growth by template identity rather than materializing applications.
        if !self.validate_value_layout_cycles() {
            // No later pass may try to materialize an application whose
            // inline layout grows forever. The validator has collected one
            // stable definition-site diagnostic for every cyclic SCC.
            return Err(self.diagnostics);
        }

        // Every nominal constraint and inheritance edge is now complete, so
        // fixed alias applications can prove both kind and nominal bounds.
        // Intrinsic owner lookup is also total, which lets exposure witnesses
        // retain the real core declaration domain for primitive targets.
        self.validate_type_alias_targets();

        // Pass 2.5: resolve function and method signatures, so calls
        // in any body see parameter and return types regardless of
        // declaration order. Interface method signatures become
        // `hir::MethodSig`s; bodyless declarations (interface and
        // abstract methods) get their parameter-only body here.
        for &(id, decl, file_index) in &pending_functions {
            self.current_file = file_index;
            self.current_owner = None;
            self.resolve_signature(id, decl);
        }
        for &(id, decl, file_index, owner) in &pending_methods {
            self.current_file = file_index;
            self.current_owner = Some(owner);
            self.resolve_method_signature(id, decl, owner);
        }
        self.current_owner = None;

        self.validate_core_operator_intrinsics(files);
        self.validate_array_conversion_intrinsics(files);
        let source_location_core = self.validate_source_location_core(files);
        let iteration_core = self.validate_iteration_core(files);
        self.iteration_core = iteration_core;

        // M10's coroutine protocol is compiler-known: MIR generation needs
        // these exact generic interfaces and intrinsic signatures rather than
        // guessing entities from names after HIR.
        let coroutine_core = self.validate_coroutine_core(files);
        let ffi_core = self.validate_ffi_core(files);
        self.ffi_core = ffi_core;
        let foreign_callback_core = self.validate_foreign_callback_core(files);
        self.foreign_callback_core = foreign_callback_core;
        self.resolve_globals(&pending_globals, &pending_objects);
        self.resolve_property_accessor_signatures();
        self.check_extension_property_signatures();
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
            &pending_objects,
            &pending_methods,
        );
        self.validate_signature_exposure();
        self.lower_export_parameter_interfaces(
            &pending_functions,
            &pending_methods,
            &pending_structs,
            &pending_classes,
            &pending_enums,
        );
        self.resolve_constructor_graphs(&pending_classes, &pending_structs, &pending_objects);
        self.lower_constructor_initialization(&pending_classes, &pending_structs, &pending_objects);
        self.declare_derived_equality_methods();
        // Compiler-generated exception edges receive complete typed class /
        // zero-argument-constructor identities after inheritance has been
        // validated and before body lowering. MIR never recovers these
        // targets from names.
        let exception_core = self.validate_exception_core(files);
        self.lower_runtime_top_level_initializers();

        // Every declaration (including generated backing classes and
        // finalized type aliases) exists before import bindings resolve;
        // bodies then see a complete import surface.
        self.resolve_import_bindings();

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
        for (id, decl, file_index, _owner) in pending_methods {
            // Abstract methods are bodyless; their parameter-only body was
            // built in pass 2.5. Interface methods with bodies are ordinary
            // default implementations and lower here.
            if matches!(self.functions[id].kind, FunctionKind::Intrinsic(_))
                || decl.modifier == ast::MethodModifier::Abstract
                || matches!(decl.body, ast::FunctionBody::None)
            {
                continue;
            }
            self.current_file = file_index;
            let body = self.lower_body(id, decl);
            self.functions[id].kind = FunctionKind::User(body);
        }
        self.lower_property_accessor_bodies();

        // Effects consume fully resolved calls and types. Local functions and
        // callable literals lifted while lowering the bodies are visible now.
        self.validate_c_ffi_types();
        self.check_generic_recursion();
        self.validate_gc_free_pointee_requirements();
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

        self.warnings.sort_by_key(|diagnostic| {
            let span = diagnostic.span.unwrap_or(Span {
                start: u32::MAX,
                end: u32::MAX,
            });
            (diagnostic.file, span.start, span.end)
        });
        if !self.diagnostics.is_empty() {
            self.diagnostics.extend(self.warnings);
            return Err(self.diagnostics);
        }
        let warnings = std::mem::take(&mut self.warnings);
        // Invariant: empty diagnostics implies `main` was found and the
        // core `Option<T>` validated above.
        let entry = entry.expect("missing `main` is always diagnosed");
        let option_core = self
            .option_core
            .expect("a missing or invalid core `Option` is always diagnosed");
        let iteration_core = iteration_core
            .expect("a missing or invalid core iteration protocol is always diagnosed");
        let coroutine_core = coroutine_core
            .expect("a missing or invalid coroutine core protocol is always diagnosed");
        let exception_core = exception_core
            .expect("a missing or invalid compiler exception core is always diagnosed");
        let ffi_core =
            ffi_core.expect("a missing or invalid FFI core protocol is always diagnosed");
        let source_location_core = source_location_core
            .expect("a missing or invalid source location core is always diagnosed");
        let public_surface = self.public_semantic_surface();
        let module = hir::Module {
            public_surface,
            source_files: self
                .intrinsic_sources
                .into_iter()
                .map(|source| hir::SourceFileMetadata {
                    provider: source.provider,
                    name: source.name,
                    source: source.source,
                })
                .collect(),
            semantic_surface: hir::SemanticSurface {
                packages: self.package_decls.clone().into_iter().collect(),
                files: self
                    .file_packages
                    .iter()
                    .zip(std::mem::take(&mut self.file_imports))
                    .map(|(package, imports)| hir::FileSurface {
                        package: *package,
                        imports,
                    })
                    .collect(),
                reexports: std::mem::take(&mut self.reexports),
            },
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
            initialization_units: self.initialization_units,
            initialization_failure_roots: self.initialization_failure_roots,
            objects: self.objects,
            object_types: self.object_types,
            companion_relations: self.companion_relations,
            singleton_values: self.singleton_values,
            singleton_published_roots: self.singleton_published_roots,
            properties: self.properties,
            extension_properties: self.extension_properties,
            property_getters: self.property_getters,
            property_setters: self.property_setters,
            delegate_storages: self.delegate_storages,
            type_aliases: self.type_aliases,
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
            boolean: self.boolean,
            string: self.string,
            option_core,
            iteration_core,
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
        };
        Ok((module, warnings))
    }
}
