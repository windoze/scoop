//! Initialization coordinators, runtime bodies, and source cycle diagnostics.

use super::*;

impl Lowerer {
    pub(crate) fn allocate_initialization_functions(
        &mut self,
        unit: hir::InitializationUnitId,
        span: ast::Span,
        file: usize,
    ) -> (hir::FunctionId, hir::FunctionId) {
        let allocate = |this: &mut Self, display_role: &str, kind| {
            let function = this.functions.alloc(Function {
                signature: hir::CallableSignature {
                    context_parameters: Vec::new(),
                    release_callability: Default::default(),
                    name: format!("$init${display_role}${}", unit.into_raw()),
                    is_suspend: false,
                    modifiers: hir::CallableModifiers::default(),
                    params: Vec::new(),
                    return_ty: this.unit,
                    attributes: hir::FunctionAttributes::default(),
                    span,
                },

                access: this.local_declaration_access(),
                genericity: hir::FunctionGenericity::Plain,
                kind,
                method: None,
            });
            this.function_files.insert(function, file);
            this.signatures.insert(
                function,
                FnSig {
                    is_suspend: false,
                    modifiers: hir::CallableModifiers::default(),
                    attributes: hir::FunctionAttributes::default(),
                    owner_type_param_count: 0,
                    type_params: Vec::new(),
                    params: Vec::new(),
                    return_ty: this.unit,
                },
            );
            this.top_level.push(function);
            function
        };
        (
            allocate(
                self,
                "body",
                FunctionKind::User(hir::Body {
                    locals: la_arena::Arena::new(),
                    statements: Vec::new(),
                }),
            ),
            allocate(self, "ensure", FunctionKind::InitializationEnsure),
        )
    }

    pub(crate) fn lower_runtime_top_level_initializers(&mut self) {
        for pending in self.pending_runtime_initializers.clone() {
            self.current_file = pending.file;
            let outer_loop_targets = std::mem::take(&mut self.loop_targets);
            let outer_source_context = self.current_source_context;
            let outer_definition_paths = std::mem::take(&mut self.definition_paths);
            let outer_definition_root = self
                .definition_root
                .replace(hir::LexicalDefinitionRoot::Function(pending.function));
            let outer_type_params = std::mem::replace(
                &mut self.type_params_in_scope,
                self.signatures[&pending.function].type_params.clone(),
            );
            self.current_return_ty = self.unit;
            self.current_fn_name = self.functions[pending.function].name.clone();
            self.push_suspension_context(SuspensionContext::Forbidden(
                ForbiddenSuspendContext::Function,
            ));
            self.push_safety_context(hir::Safety::Safe);
            self.current_owner = None;
            self.current_this = None;
            self.set_source_context(hir::SourceContextSubject::Function(pending.function));
            self.push_scope();
            self.current_initialization_unit = Some(pending.unit);

            let mut statements = Vec::new();
            let mut sink = Vec::new();
            let target = match &pending.kind {
                PendingRuntimeInitializerKind::Stored { storage, .. }
                | PendingRuntimeInitializerKind::Delegated {
                    storage: PendingDelegateStorage::Global { storage, .. },
                    ..
                } => hir::AssignTarget::Global(*storage),
                PendingRuntimeInitializerKind::Delegated {
                    storage: PendingDelegateStorage::Generic(template),
                    ..
                } => hir::AssignTarget::GenericDelegateStorage(
                    self.generic_delegate_reference(*template, pending.function),
                ),
            };
            let value = match &pending.kind {
                PendingRuntimeInitializerKind::Stored { ty, expression, .. } => {
                    self.lower_expr(expression, &mut sink, Some(*ty))
                        .and_then(|value| {
                            if self.is_subtype(value.ty, *ty) {
                                Some(self.adapt_to(value, *ty))
                            } else {
                                let expected = self.type_name(*ty);
                                let found = self.type_name(value.ty);
                                self.error(
                                    expression.span(),
                                    format!(
                                        "top-level property initializer must be of type {expected}, found {found}"
                                    ),
                                );
                                None
                            }
                        })
                }
                PendingRuntimeInitializerKind::Delegated {
                    property,
                    storage,
                    expression,
                } => self
                    .lower_expr(expression, &mut sink, None)
                    .and_then(|delegate| {
                        match self.resolve_delegate_role_call(
                            delegate.clone(),
                            hir::PropertyDelegateOperatorKind::ProvideDelegate,
                            Vec::new(),
                            pending.span,
                        ) {
                            crate::properties::DelegateRoleCall::Resolved(effective) => {
                                Some(effective.expression)
                            }
                            crate::properties::DelegateRoleCall::NoApplicable => Some(delegate),
                            crate::properties::DelegateRoleCall::Failed => None,
                        }
                    })
                    .inspect(|effective| {
                        match *storage {
                            PendingDelegateStorage::Global { storage, delegate } => {
                                self.globals[storage].ty = effective.ty;
                                self.delegate_storages[delegate].ty = effective.ty;
                                debug_assert_eq!(self.delegate_storages[delegate].property, *property);
                            }
                            PendingDelegateStorage::Generic(template) => {
                                self.generic_delegate_templates[template].ty = effective.ty;
                                debug_assert_eq!(self.generic_delegate_templates[template].property, *property);
                            }
                        }
                    }),
            };
            if let Some(value) = value {
                statements.extend(sink);
                statements.push(hir::Statement {
                    kind: hir::StatementKind::Assign { target, value },
                    span: pending.span,
                });
            }
            self.current_initialization_unit = None;
            self.pop_scope();
            self.current_source_context = outer_source_context;
            self.definition_paths = outer_definition_paths;
            self.definition_root = outer_definition_root;
            self.pop_safety_context();
            self.pop_suspension_context();
            self.functions[pending.function].kind = FunctionKind::User(hir::Body {
                locals: std::mem::take(&mut self.locals),
                statements,
            });
            debug_assert!(self.loop_targets.is_empty());
            self.loop_targets = outer_loop_targets;
            self.type_params_in_scope = outer_type_params;
        }
        self.diagnose_initialization_cycles();
    }

    pub(super) fn diagnose_initialization_cycles(&mut self) {
        let mut states = vec![0_u8; self.initialization_units.len()];
        let mut stack = Vec::new();
        for raw in 0..self.initialization_units.len() {
            let unit = hir::InitializationUnitId::from_raw((raw as u32).into());
            self.visit_initialization_unit(unit, &mut states, &mut stack);
        }
    }

    pub(super) fn initialization_source_display(&self, file: usize) -> String {
        let path = Path::new(&self.intrinsic_sources[file].name);
        let mut components = Vec::new();
        for component in path.components() {
            match component {
                Component::Normal(value) => components.push(value.to_string_lossy().into_owned()),
                Component::ParentDir => components.push("__parent__".to_string()),
                Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
            }
        }
        if path.is_absolute() {
            components.pop().unwrap_or_else(|| "<user>".to_string())
        } else if components.is_empty() {
            "<user>".to_string()
        } else {
            components.join("/")
        }
    }

    pub(crate) fn initialization_property_display_name(
        &self,
        file: usize,
        owner: hir::PropertyOwner,
        access: &hir::DeclarationAccess,
        name: &str,
    ) -> String {
        let private = access.declared == hir::DeclaredVisibility::Private;
        match owner {
            hir::PropertyOwner::TopLevel if private => format!(
                "top-level-private:{}:{name}",
                self.initialization_source_display(file)
            ),
            hir::PropertyOwner::TopLevel => format!("top-level:{name}"),
            hir::PropertyOwner::Extension(extension) => {
                let receiver = self.type_name(self.extension_properties[extension].receiver_ty);
                if private {
                    format!(
                        "extension-private:{}:{receiver}:{name}",
                        self.initialization_source_display(file)
                    )
                } else {
                    format!("extension:{receiver}:{name}")
                }
            }
            hir::PropertyOwner::Class(_)
            | hir::PropertyOwner::Struct(_)
            | hir::PropertyOwner::Enum(_)
            | hir::PropertyOwner::Interface(_)
            | hir::PropertyOwner::Object(_) => {
                unreachable!("only top-level and extension properties own initialization units")
            }
        }
    }

    pub(crate) fn singleton_initialization_display_name(&self, object: hir::ObjectId) -> String {
        let declaration = &self.objects[object];
        let qualified_name = crate::Owner::Object(object).describe_name(self);
        match declaration.kind {
            hir::ObjectKind::Companion(_) => format!("companion:{qualified_name}"),
            hir::ObjectKind::Standalone
                if declaration.access.declared == hir::DeclaredVisibility::Private
                    && declaration.owner.is_none() =>
            {
                format!(
                    "object-private:{}:{qualified_name}",
                    self.initialization_source_display(self.object_files[&object])
                )
            }
            hir::ObjectKind::Standalone => format!("object:{qualified_name}"),
        }
    }

    pub(super) fn visit_initialization_unit(
        &mut self,
        unit: hir::InitializationUnitId,
        states: &mut [u8],
        stack: &mut Vec<hir::InitializationUnitId>,
    ) {
        let index = unit.into_raw().into_u32() as usize;
        if states[index] != 0 {
            return;
        }
        states[index] = 1;
        stack.push(unit);
        for dependency in self.initialization_units[unit].dependencies.clone() {
            let dependency_index = dependency.unit.into_raw().into_u32() as usize;
            if states[dependency_index] == 0 {
                self.visit_initialization_unit(dependency.unit, states, stack);
            } else if states[dependency_index] == 1 {
                let start = stack
                    .iter()
                    .position(|candidate| *candidate == dependency.unit)
                    .expect("an active dependency is present in the DFS stack");
                let mut path = stack[start..]
                    .iter()
                    .map(|candidate| self.initialization_units[*candidate].display_name.clone())
                    .collect::<Vec<_>>();
                path.push(
                    self.initialization_units[dependency.unit]
                        .display_name
                        .clone(),
                );
                self.current_file = match self.initialization_units[unit].kind {
                    hir::InitializationUnitKind::EagerTopLevel { property, .. }
                    | hir::InitializationUnitKind::GenericDelegatedExtension { property, .. } => {
                        self.property_files[&property]
                    }
                    hir::InitializationUnitKind::LazySingleton { value, .. } => {
                        let object = self.singleton_values[value].declaration;
                        self.object_files[&object]
                    }
                };
                self.error(
                    dependency.span,
                    format!("initialization cycle: {}", path.join(" -> ")),
                );
            }
        }
        stack.pop();
        states[index] = 2;
    }
}
