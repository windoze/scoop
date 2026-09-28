use super::*;

impl Lowerer {
    pub(crate) fn materialize_imported_callable_body(
        &mut self,
        id: hir::ImportedGenericCallableTemplateId,
        template: &crate::imported_generics::PreparedImportedGeneric,
    ) -> Result<hir::Body, ImportedDefaultMaterializationError> {
        let saved_locals = std::mem::replace(&mut self.locals, template.locals.clone());
        let mut context = ImportedDefaultContext {
            owner: ImportedTemplateSource::Callable(&template.source),
            callables: &BTreeMap::new(),
            bindings: &template.bindings,
            locals: template
                .locals
                .iter()
                .map(|(id, local)| {
                    (
                        local.selector.clone(),
                        hir::Expr {
                            kind: hir::ExprKind::Local(id),
                            ty: local.ty,
                            span: template.span,
                            origin: hir::ExpressionOrigin::Definition(template.origin),
                        },
                    )
                })
                .collect(),
            captures: match &template.declaration {
                hir::ImportedCallableTemplateOrigin::Closure {
                    capture_bindings, ..
                } => capture_bindings,
                _ => &[],
            },
            loop_targets: Vec::new(),
            evaluation: ImportedTemplateEvaluation::Definition(
                hir::ImportedCallableTemplateParent::Function(id),
            ),
        };
        let statements = template
            .source
            .body()
            .statements()
            .iter()
            .filter_map(|statement| {
                self.materialize_imported_default_statement(statement, &mut context)
                    .transpose()
            })
            .collect::<Result<Vec<_>, _>>();
        let locals = std::mem::replace(&mut self.locals, saved_locals);
        statements.map(|statements| hir::Body { locals, statements })
    }

    pub(super) fn imported_template_call_kind(
        &mut self,
        callee: &hir::DefaultCallableRefV1,
        args: Vec<hir::Expr>,
        receiver: hir::SourceCallReceiver<hir::TypeId>,
        kind: MemberCallKind,
        context: &ImportedDefaultContext<'_>,
    ) -> Result<hir::ExprKind, ImportedDefaultMaterializationError> {
        Ok(
            match self.materialize_imported_callable_target(callee, kind, context)? {
                hir::ImportedCallableTarget::Application(application) => {
                    hir::ExprKind::ImportedGenericCall {
                        application,
                        kind: match kind {
                            MemberCallKind::Ordinary => hir::ImportedGenericCallKind::Ordinary,
                            MemberCallKind::DirectSuper => {
                                hir::ImportedGenericCallKind::DirectSuper
                            }
                        },
                        binding: None,
                        args,
                        receiver,
                    }
                }
                hir::ImportedCallableTarget::Dependency(callee) => {
                    hir::ExprKind::ImportedDependencyCall {
                        callee,
                        binding: None,
                        args,
                        receiver,
                    }
                }
            },
        )
    }

    pub(super) fn materialize_imported_callable_target(
        &mut self,
        callee: &hir::DefaultCallableRefV1,
        kind: MemberCallKind,
        context: &ImportedDefaultContext<'_>,
    ) -> Result<hir::ImportedCallableTarget, ImportedDefaultMaterializationError> {
        let origin = super::super::plan::default_callable_origin(callee)
            .map_err(|error| ImportedDefaultMaterializationError::Plan(error.to_string()))?;
        let local = self
            .request_imported_local_function(origin)
            .map_err(ImportedDefaultMaterializationError::Plan)?;
        let owner = match callee.owner() {
            scoop_identity::OptionalSignatureType::Present(owner) => {
                let owner = self
                    .imported_generic_type(owner, context.bindings)
                    .map_err(ImportedDefaultMaterializationError::Plan)?;
                Some(owner)
            }
            scoop_identity::OptionalSignatureType::Absent => None,
        };
        let template = if let Some(template) = local {
            Some(template)
        } else if matches!(
            callee.declaration(),
            hir::DefaultCallableDeclarationV1::GenericFunction(_)
        ) || matches!(
            callee.declaration(),
            hir::DefaultCallableDeclarationV1::PropertyAccessor(_)
        ) && !callee.type_arguments().is_empty()
            || owner.is_some_and(|owner| {
                self.types[owner]
                    .imported_nominal_application()
                    .is_some_and(|(_, arguments)| !arguments.is_empty())
            })
        {
            let declaration = self
                .dependencies
                .as_ref()
                .expect("dependency body retains its source catalog")
                .callable_declaration(origin)
                .map_err(|error| {
                    ImportedDefaultMaterializationError::DependencySelection(error.to_string())
                })?;
            Some(
                self.request_imported_generic_template(declaration)
                    .map_err(ImportedDefaultMaterializationError::Plan)?,
            )
        } else {
            None
        };
        if let Some(template) = template {
            let arguments = callee
                .type_arguments()
                .iter()
                .map(|key| {
                    self.imported_default_type_with_bindings(key, context.bindings)
                        .map_err(|error| {
                            ImportedDefaultMaterializationError::Plan(error.to_string())
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let arguments = if let Some(owner) = owner {
                hir::ImportedCallableArguments::Method {
                    owner,
                    method_arguments: arguments,
                }
            } else {
                hir::ImportedCallableArguments::Function(
                    hir::NonEmptyVec::from_vec(arguments).ok_or_else(|| {
                        ImportedDefaultMaterializationError::Plan(
                            "generic dependency call has no arguments".into(),
                        )
                    })?,
                )
            };
            let application =
                self.imported_generic_applications
                    .alloc(hir::ImportedGenericCallableApplication {
                        template,
                        arguments,
                    });
            return Ok(hir::ImportedCallableTarget::Application(application));
        }
        self.imported_default_callable_target(origin, kind, context)
            .map(hir::ImportedCallableTarget::Dependency)
    }
}
