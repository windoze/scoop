use super::*;

/// Original declarations and their allocated identities, shared by later passes.
pub(crate) struct SourceDeclarations<'a> {
    pub(super) current_cone: scoop_identity::ConeIdentity,
    pub(super) source_headers: Vec<ast::SourceHeaderRef<'a>>,
    pub(super) pending_structs: Vec<(StructId, ast::StructSource<'a>, usize)>,
    pub(super) pending_enums: Vec<(EnumId, ast::EnumSource<'a>, usize)>,
    pub(super) pending_classes: Vec<(ClassId, ast::ClassSource<'a>, usize)>,
    pub(super) pending_interfaces: Vec<(InterfaceId, ast::InterfaceSource<'a>, usize)>,
    pub(super) pending_objects: Vec<(hir::ObjectId, crate::declarations::ObjectSource<'a>, usize)>,
    pub(super) function_headers: Vec<crate::declarations::FunctionHeaderInput<'a>>,
    pub(super) pending_functions: Vec<(FunctionId, &'a ast::FunctionDecl, usize)>,
    pub(super) recovered_functions: Vec<(FunctionId, &'a ast::RecoveredFunctionBody)>,
    pub(super) pending_globals: Vec<(&'a ast::GlobalDecl, usize)>,
    pub(super) method_headers: Vec<crate::declarations::MethodHeaderInput<'a>>,
    pub(super) pending_methods: Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
}

pub(super) struct SignatureCore {
    pub(super) intrinsic_type_core: Option<hir::IntrinsicTypeCore>,
    pub(super) source_location_core: Option<hir::SourceLocationCore>,
    pub(super) iteration_core: Option<hir::IterationCore>,
    pub(super) coroutine_core: Option<hir::CoroutineCore>,
    pub(super) ffi_core: Option<hir::FfiCore>,
    pub(super) foreign_callback_core: Option<hir::ForeignCallbackCore>,
}

pub(super) struct DeclarationBodyCore {
    pub(super) program_arguments: Option<FunctionId>,
    pub(super) type_reference: Option<FunctionId>,
    pub(super) exception_core: Option<hir::CompilerExceptionCore>,
}

impl Lowerer {
    #[cfg(test)]
    pub(super) fn collect_source_declarations<'a>(
        &mut self,
        files: &'a [ast::SourceFile],
        world: Option<&hir::ImportedSemanticWorld<'_>>,
    ) -> Result<SourceDeclarations<'a>, Vec<Diagnostic>> {
        self.collect_declaration_inputs(
            files.iter().map(ast::SourceFile::header).collect(),
            files.iter().enumerate().flat_map(|(file, source)| {
                source
                    .declarations
                    .iter()
                    .map(move |declaration| (file, ast::DeclarationInput::Complete(declaration)))
            }),
            world,
        )
    }

    #[cfg(test)]
    pub(crate) fn collect_declaration_inputs<'a>(
        &mut self,
        source_headers: Vec<ast::SourceHeaderRef<'a>>,
        inputs: impl IntoIterator<Item = (usize, ast::DeclarationInput<'a>)>,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
    ) -> Result<SourceDeclarations<'a>, Vec<Diagnostic>> {
        self.collect_declaration_inputs_with_cancellation(
            source_headers,
            inputs,
            world,
            &cancellation::uninterrupted,
        )
        .map_err(PhaseFailure::into_diagnostics)
    }

    pub(super) fn collect_declaration_inputs_with_cancellation<'a, C>(
        &mut self,
        source_headers: Vec<ast::SourceHeaderRef<'a>>,
        inputs: impl IntoIterator<Item = (usize, ast::DeclarationInput<'a>)>,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
        check: &impl Fn() -> Result<(), C>,
    ) -> Result<SourceDeclarations<'a>, PhaseFailure<C>> {
        PhaseFailure::check(check)?;
        if source_headers.is_empty() {
            return Err(vec![Diagnostic::without_span(
                ast::DiagnosticSeverity::Error,
                0,
                "no source files to compile",
            )]
            .into());
        }
        assert_eq!(
            source_headers.len(),
            self.intrinsic_sources.len(),
            "every parsed source has exactly one lowering source descriptor"
        );
        let current_cone = self.current_cone();
        self.top_level_namespaces.initialize_sources(
            self.intrinsic_sources
                .iter()
                .map(|source| (source.identity.cone(), source.role)),
            current_cone,
            &source_headers,
        );

        // Pass 1: declare aliases, nominals and functions across all files
        // (core first), so bodies and field types resolve regardless of
        // declaration order. Aliases and top-level nominals share the type
        // namespace and must not collide; functions occupy a separate
        // namespace where one name may collect several overloads (M7), and
        // member functions live in per-owner namespaces.
        let mut pending_annotations = Vec::new();
        let mut pending_structs = Vec::new();
        let mut pending_enums = Vec::new();
        let mut pending_classes = Vec::new();
        let mut pending_interfaces = Vec::new();
        let mut pending_objects = Vec::new();
        let mut pending_functions = Vec::new();
        let mut function_headers = Vec::new();
        let mut recovered_functions = Vec::new();
        let mut unavailable = Vec::new();
        let mut pending_globals = Vec::new();
        let mut pending_methods: Vec<(FunctionId, &ast::FunctionDecl, usize, Owner)> = Vec::new();
        for (file_index, input) in inputs {
            PhaseFailure::check(check)?;
            self.current_file = file_index;
            let is_core = self.source_is_core(file_index);
            let decl = match input {
                ast::DeclarationInput::Complete(declaration) => declaration,
                ast::DeclarationInput::RecoveredNominal(declaration) => {
                    match declaration {
                        ast::RecoveredNominal::Class(declaration) => {
                            self.declare_class(
                                declaration.source(),
                                is_core,
                                &mut pending_classes,
                                &mut pending_methods,
                                file_index,
                                None,
                            );
                        }
                        ast::RecoveredNominal::Struct(declaration) => {
                            self.declare_struct(
                                declaration.source(),
                                &mut pending_structs,
                                &mut pending_methods,
                                file_index,
                                None,
                            );
                        }
                        ast::RecoveredNominal::Interface(declaration) => {
                            self.declare_interface(
                                declaration.source(),
                                &mut pending_interfaces,
                                &mut pending_methods,
                                file_index,
                                None,
                            );
                        }
                        ast::RecoveredNominal::Enum(declaration) => {
                            self.declare_enum(
                                declaration.source(),
                                is_core,
                                &mut pending_enums,
                                &mut pending_methods,
                                file_index,
                                None,
                            );
                        }
                    }
                    continue;
                }
                ast::DeclarationInput::Unavailable(header) => {
                    unavailable.push((header, file_index));
                    continue;
                }
                ast::DeclarationInput::FunctionHeader(header) => {
                    let id = self.declare_function_header(header, file_index);
                    function_headers.push((id, header, file_index));
                    continue;
                }
                ast::DeclarationInput::RecoveredFunction { header, body } => {
                    let id = self.declare_function_header(header, file_index);
                    function_headers.push((id, header, file_index));
                    recovered_functions.push((id, body));
                    continue;
                }
            };
            match decl {
                ast::Decl::AnnotationClass(decl) => {
                    pending_annotations.push((decl, file_index, None))
                }
                ast::Decl::Global(decl) => pending_globals.push((decl, file_index)),
                ast::Decl::Struct(decl) => {
                    let _ = self.declare_struct(
                        decl.source(),
                        &mut pending_structs,
                        &mut pending_methods,
                        file_index,
                        None,
                    );
                }
                ast::Decl::Enum(decl) => {
                    let _ = self.declare_enum(
                        decl.source(),
                        is_core,
                        &mut pending_enums,
                        &mut pending_methods,
                        file_index,
                        None,
                    );
                }
                ast::Decl::Class(decl) => {
                    let _ = self.declare_class(
                        decl.source(),
                        is_core,
                        &mut pending_classes,
                        &mut pending_methods,
                        file_index,
                        None,
                    );
                }
                ast::Decl::Interface(decl) => {
                    let _ = self.declare_interface(
                        decl.source(),
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
            annotations: &mut pending_annotations,
            structs: &mut pending_structs,
            enums: &mut pending_enums,
            classes: &mut pending_classes,
            interfaces: &mut pending_interfaces,
            objects: &mut pending_objects,
            methods: &mut pending_methods,
        };
        for (id, declaration, file) in root_structs {
            PhaseFailure::check(check)?;
            self.declare_struct_nested(Owner::Struct(id), declaration, &mut nested_queues, file);
        }
        for (id, declaration, file) in root_enums {
            PhaseFailure::check(check)?;
            self.declare_enum_nested(Owner::Enum(id), declaration, &mut nested_queues, file);
        }
        for (id, declaration, file) in root_classes {
            PhaseFailure::check(check)?;
            self.declare_class_nested(Owner::Class(id), declaration, &mut nested_queues, file);
        }
        for (id, declaration, file) in root_interfaces {
            PhaseFailure::check(check)?;
            self.declare_interface_nested(
                Owner::Interface(id),
                declaration,
                &mut nested_queues,
                file,
            );
        }
        for (id, declaration, file) in root_objects {
            PhaseFailure::check(check)?;
            self.declare_object_nested(Owner::Object(id), declaration, &mut nested_queues, file);
        }

        PhaseFailure::check(check)?;
        if let Err(error) = self.establish_nominal_identities() {
            let mut diagnostic = Diagnostic::at(error.span(), error.to_string());
            diagnostic.file = error.file();
            return Err(vec![diagnostic].into());
        }

        for (declaration, file, owner) in pending_annotations {
            PhaseFailure::check(check)?;
            self.current_file = file;
            self.declare_annotation_class(declaration, file, owner);
        }

        function_headers.extend(
            pending_functions
                .iter()
                .map(|&(id, declaration, file)| (id, declaration.header(), file)),
        );
        let mut method_headers = pending_methods
            .iter()
            .map(|&(id, declaration, file, owner)| (id, declaration.header(), file, owner))
            .collect::<Vec<_>>();
        let recovered_nominals =
            pending_classes
                .iter()
                .map(|(id, declaration, file)| (Owner::Class(*id), declaration.recovery, *file))
                .chain(pending_structs.iter().map(|(id, declaration, file)| {
                    (Owner::Struct(*id), declaration.recovery, *file)
                }))
                .chain(pending_interfaces.iter().map(|(id, declaration, file)| {
                    (Owner::Interface(*id), declaration.recovery, *file)
                }))
                .chain(pending_enums.iter().map(|(id, declaration, file)| {
                    (Owner::Enum(*id), declaration.recovery, *file)
                }));
        for (owner, recovery, file) in recovered_nominals {
            for method in &recovery.methods {
                PhaseFailure::check(check)?;
                self.current_file = file;
                let header = method.header();
                let function = self.declare_method_header(header, owner, file);
                method_headers.push((function, header, file, owner));
                if let Some(body) = &method.body {
                    recovered_functions.push((function, body));
                }
            }
            if !recovery.methods.is_empty() {
                let methods = match owner {
                    Owner::Class(class) => &mut self.classes[class].methods,
                    Owner::Struct(structure) => &mut self.structs[structure].methods,
                    Owner::Enum(enumeration) => &mut self.enums[enumeration].methods,
                    Owner::Interface(interface) => {
                        let declaration = &mut self.interfaces[interface];
                        declaration.methods.sort_by_key(|member| {
                            self.functions[self.interface_method_entities[*member].function]
                                .span
                                .start
                        });
                        declaration
                            .private_methods
                            .sort_by_key(|function| self.functions[*function].span.start);
                        self.interface_methods.get_mut(&interface).unwrap()
                    }
                    Owner::Object(_) => unreachable!("recovered nominal inputs"),
                };
                methods.sort_by_key(|function| self.functions[*function].span.start);
            }
        }

        PhaseFailure::check(check)?;
        let errors_before_imports = self.diagnostics.len();
        self.collect_and_resolve_imports(
            &source_headers,
            crate::imports::ImportDeclarationInputs {
                functions: &function_headers,
                unavailable: &unavailable,
                methods: &method_headers,
                properties: &pending_globals,
                enumerations: &pending_enums,
                objects: &pending_objects,
            },
            world,
        );
        PhaseFailure::check(check)?;
        if self.diagnostics.len() != errors_before_imports {
            return Err(self.take_source_diagnostics().into());
        }

        Ok(SourceDeclarations {
            current_cone,
            source_headers,
            pending_structs,
            pending_enums,
            pending_classes,
            pending_interfaces,
            pending_objects,
            function_headers,
            recovered_functions,
            pending_functions,
            pending_globals,
            method_headers,
            pending_methods,
        })
    }
}
