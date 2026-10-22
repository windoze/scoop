use super::*;

/// Original declarations and their allocated identities, shared by later passes.
pub(crate) struct SourceDeclarations<'a> {
    pub(super) current_cone: scoop_identity::ConeIdentity,
    pub(super) source_headers: Vec<ast::SourceHeaderRef<'a>>,
    pub(super) pending_structs: Vec<(StructId, &'a ast::StructDecl, usize)>,
    pub(super) pending_enums: Vec<(EnumId, &'a ast::EnumDecl, usize)>,
    pub(super) pending_classes: Vec<(ClassId, &'a ast::ClassDecl, usize)>,
    pub(super) pending_interfaces: Vec<(InterfaceId, &'a ast::InterfaceDecl, usize)>,
    pub(super) pending_objects: Vec<(hir::ObjectId, crate::declarations::ObjectSource<'a>, usize)>,
    pub(super) function_headers: Vec<crate::declarations::FunctionHeaderInput<'a>>,
    pub(super) pending_functions: Vec<(FunctionId, &'a ast::FunctionDecl, usize)>,
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

    pub(crate) fn collect_declaration_inputs<'a>(
        &mut self,
        source_headers: Vec<ast::SourceHeaderRef<'a>>,
        inputs: impl IntoIterator<Item = (usize, ast::DeclarationInput<'a>)>,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
    ) -> Result<SourceDeclarations<'a>, Vec<Diagnostic>> {
        if source_headers.is_empty() {
            return Err(vec![Diagnostic::without_span(
                ast::DiagnosticSeverity::Error,
                0,
                "no source files to compile",
            )]);
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
        let mut unavailable = Vec::new();
        let mut pending_globals = Vec::new();
        let mut pending_methods: Vec<(FunctionId, &ast::FunctionDecl, usize, Owner)> = Vec::new();
        for (file_index, input) in inputs {
            self.current_file = file_index;
            let is_core = self.source_is_core(file_index);
            let decl = match input {
                ast::DeclarationInput::Complete(declaration) => declaration,
                ast::DeclarationInput::Unavailable(header) => {
                    unavailable.push((header, file_index));
                    continue;
                }
                ast::DeclarationInput::FunctionHeader(header) => {
                    let id = self.declare_function_header(header, file_index);
                    function_headers.push((id, header, file_index));
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

        if let Err(error) = self.establish_nominal_identities() {
            let mut diagnostic = Diagnostic::at(error.span(), error.to_string());
            diagnostic.file = error.file();
            return Err(vec![diagnostic]);
        }

        for (declaration, file, owner) in pending_annotations {
            self.current_file = file;
            self.declare_annotation_class(declaration, file, owner);
        }

        function_headers.extend(
            pending_functions
                .iter()
                .map(|&(id, declaration, file)| (id, declaration.header(), file)),
        );
        let method_headers = pending_methods
            .iter()
            .map(|&(id, declaration, file, owner)| (id, declaration.header(), file, owner))
            .collect::<Vec<_>>();

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
        if self.diagnostics.len() != errors_before_imports {
            return Err(self.take_source_diagnostics());
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
            pending_functions,
            pending_globals,
            method_headers,
            pending_methods,
        })
    }
}
