use super::*;

impl Lowerer {
    #[cfg(test)]
    pub(crate) fn run_defined(
        self,
        files: &[ast::SourceFile],
    ) -> Result<(hir::Module, Vec<Diagnostic>), Vec<Diagnostic>> {
        let dependencies = hir::ImportedDependencySelectionPlan::empty(self.current_cone());
        let (module, warnings, _) = self
            .with_imported_dependencies(dependencies)
            .run(files, None)?;
        Ok((module, warnings))
    }

    pub(crate) fn run_with_dependencies(
        self,
        files: &[ast::SourceFile],
        world: &hir::ImportedSemanticWorld<'_>,
    ) -> Result<(hir::Module, Vec<Diagnostic>, LoweringCompletion), Vec<Diagnostic>> {
        self.run(files, Some(world))
    }

    fn run(
        mut self,
        files: &[ast::SourceFile],
        world: Option<&hir::ImportedSemanticWorld<'_>>,
    ) -> Result<(hir::Module, Vec<Diagnostic>, LoweringCompletion), Vec<Diagnostic>> {
        if files.is_empty() {
            return Err(vec![Diagnostic::without_span(
                ast::DiagnosticSeverity::Error,
                0,
                "no source files to compile",
            )]);
        }
        assert_eq!(
            files.len(),
            self.intrinsic_sources.len(),
            "every parsed source has exactly one lowering source descriptor"
        );
        let primary_output_file = self.primary_output_file();
        let current_cone = self.current_cone();
        let defines_core = matches!(self.core, CoreLoweringAuthority::Defined);
        let core_diagnostic_file = self.core_diagnostic_file();
        self.top_level_namespaces.initialize_sources(
            self.intrinsic_sources
                .iter()
                .map(|source| source.identity.cone()),
            current_cone,
            files,
        );

        // Pass 1: declare aliases, nominals and functions across all files
        // (core first), so bodies and field types resolve regardless of
        // declaration order. Aliases and top-level nominals share the type
        // namespace and must not collide; functions occupy a separate
        // namespace where one name may collect several overloads (M7), and
        // member functions live in per-owner namespaces.
        let mut pending_structs = Vec::new();
        let mut pending_enums = Vec::new();
        let mut pending_classes = Vec::new();
        let mut pending_interfaces = Vec::new();
        let mut pending_objects = Vec::new();
        let mut pending_functions = Vec::new();
        let mut pending_globals = Vec::new();
        let mut pending_methods: Vec<(FunctionId, &ast::FunctionDecl, usize, Owner)> = Vec::new();
        for (file_index, file) in files.iter().enumerate() {
            self.current_file = file_index;
            let is_core = self.source_is_core(file_index);
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

        let errors_before_imports = self.diagnostics.len();
        self.collect_and_resolve_imports(
            files,
            crate::imports::ImportDeclarationInputs {
                functions: &pending_functions,
                methods: &pending_methods,
                properties: &pending_globals,
                enumerations: &pending_enums,
                objects: &pending_objects,
            },
            world,
        );
        if self.diagnostics.len() != errors_before_imports {
            return Err(self.diagnostics);
        }

        // Alias targets may mention any declaration in the Cone, including a
        // later alias, `Option<T>` through nullable syntax, and the special
        // pointer families. Establish those source identities before the
        // alias graph is expanded. Application bounds are checked again once
        // every nominal constraint is complete below.
        if defines_core {
            self.ffi_ptr = self.require_core_struct("Ptr", files);
            self.ffi_fun_ptr = self.require_core_struct("FunPtr", files);
            self.ffi_pinned_ptr = self.require_core_struct("PinnedPtr", files);
            self.ffi_gc_handle = self.require_core_struct("GcHandle", files);
            self.ffi_foreign_callback = self.require_core_struct("ForeignCallback", files);
            self.validate_option_enum(files);
        }
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
        let intrinsic_type_core = if defines_core {
            self.validate_intrinsic_type_core(files)
        } else {
            None
        };
        if let Some(core) = intrinsic_type_core {
            if self.ffi_ptr != Some(core.ptr) {
                self.current_file = primary_output_file;
                self.error(
                    files[core_diagnostic_file].span,
                    "the `Ptr` FFI core owner must be the `core_ptr` intrinsic type".to_string(),
                );
            }
            if self.ffi_fun_ptr != Some(core.fun_ptr) {
                self.current_file = primary_output_file;
                self.error(
                    files[core_diagnostic_file].span,
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
        if defines_core {
            self.validate_throwable(files);
        }

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
        if defines_core {
            self.validate_option_variants();
        }
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

        if defines_core {
            self.validate_core_operator_intrinsics(files);
            self.validate_array_conversion_intrinsics(files);
            self.validate_gc_control_intrinsics(files);
        }
        let source_location_core = if defines_core {
            self.validate_source_location_core(files)
        } else {
            None
        };
        let iteration_core = if defines_core {
            self.validate_iteration_core(files)
        } else {
            None
        };
        self.iteration_core = iteration_core;

        // M10's coroutine protocol is compiler-known: MIR generation needs
        // these exact generic interfaces and intrinsic signatures rather than
        // guessing entities from names after HIR.
        let coroutine_core = if defines_core {
            self.validate_coroutine_core(files)
        } else {
            None
        };
        let ffi_core = if defines_core {
            self.validate_ffi_core(files)
        } else {
            None
        };
        self.ffi_core = ffi_core;
        let foreign_callback_core = if defines_core {
            self.validate_foreign_callback_core(files)
        } else {
            None
        };
        self.foreign_callback_core = foreign_callback_core;
        self.resolve_globals(&pending_globals, &pending_objects);
        self.finalize_import_targets();
        self.resolve_property_accessor_signatures();
        self.check_extension_property_signatures();
        self.validate_extern_functions();
        self.validate_extern_global_symbols();

        // Publish the complete duplicate-signature rejection set after all
        // related signatures are final and before body-capable passes may
        // perform semantic callable lookup.
        self.validate_and_freeze_duplicate_signatures(&pending_functions, &pending_methods);
        assert!(
            self.declaration_surface.is_frozen(),
            "body-capable passes require a frozen declaration surface"
        );

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
        // Defaults and constructor expressions can select derived equality.
        // Publish its conditional signatures before any of those bodies lower.
        self.declare_derived_equality_methods();
        self.lower_export_parameter_interfaces(
            &pending_functions,
            &pending_methods,
            &pending_structs,
            &pending_classes,
            &pending_enums,
        );
        self.resolve_constructor_graphs(&pending_classes, &pending_structs, &pending_objects);
        self.lower_constructor_initialization(&pending_classes, &pending_structs, &pending_objects);
        // Compiler-generated exception edges receive complete typed class /
        // zero-argument-constructor identities after inheritance has been
        // validated and before body lowering. MIR never recovers these
        // targets from names.
        let exception_core = if defines_core {
            self.validate_exception_core(files)
        } else {
            None
        };
        if defines_core && (self.option_core.is_none() || exception_core.is_none()) {
            return Err(self.diagnostics);
        }
        self.lower_runtime_top_level_initializers();

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
        // Protocol provenance and dependency selections are independent.
        let core_protocols = match self.core.clone() {
            CoreLoweringAuthority::Defined => {
                hir::CoreProtocols::Defined(Box::new(hir::DefinedCoreProtocols {
                    option: self
                        .option_core
                        .expect("a missing or invalid core `Option` is always diagnosed"),
                    iteration: iteration_core
                        .expect("a missing or invalid core iteration protocol is always diagnosed"),
                    exceptions: exception_core
                        .expect("a missing or invalid compiler exception core is always diagnosed"),
                    coroutines: coroutine_core
                        .expect("a missing or invalid coroutine core protocol is always diagnosed"),
                    ffi: ffi_core
                        .expect("a missing or invalid FFI core protocol is always diagnosed"),
                    foreign_callbacks: foreign_callback_core.expect(
                        "a missing or invalid foreign callback core protocol is always diagnosed",
                    ),
                    fundamental_types: intrinsic_type_core
                        .expect("missing or invalid intrinsic core types are always diagnosed"),
                    source_location: source_location_core
                        .expect("a missing or invalid source location core is always diagnosed"),
                }))
            }
            CoreLoweringAuthority::Imported(authority) => {
                hir::CoreProtocols::Imported(Box::new(authority.protocols))
            }
        };
        let completion = LoweringCompletion {
            dependencies: self
                .dependencies
                .take()
                .expect("every HIR entry installs its dependency selection plan"),
            binding_witness_uses: std::mem::take(&mut self.retained_binding_witness_uses),
        };
        self.finish(current_cone, warnings, core_protocols, completion)
    }
}
