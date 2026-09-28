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
        let origin = super::super::plan::default_callable_origin(callee)
            .map_err(|error| ImportedDefaultMaterializationError::Plan(error.to_string()))?;
        let local = self
            .request_imported_local_function(origin)
            .map_err(ImportedDefaultMaterializationError::Plan)?;
        let owner_arguments = match callee.owner() {
            scoop_identity::OptionalSignatureType::Present(owner) => {
                let owner = self
                    .imported_generic_type(owner, context.bindings)
                    .map_err(ImportedDefaultMaterializationError::Plan)?;
                self.types[owner]
                    .imported_nominal_application()
                    .map(|(_, arguments)| arguments.to_vec())
                    .unwrap_or_default()
            }
            scoop_identity::OptionalSignatureType::Absent => Vec::new(),
        };
        let template = if let Some(template) = local {
            Some(template)
        } else if matches!(
            callee.declaration(),
            hir::DefaultCallableDeclarationV1::GenericFunction(_)
        ) || !owner_arguments.is_empty()
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
            let mut arguments = owner_arguments;
            arguments.extend(
                callee
                    .type_arguments()
                    .iter()
                    .map(|key| {
                        self.imported_default_type_with_bindings(key, context.bindings)
                            .map_err(|error| {
                                ImportedDefaultMaterializationError::Plan(error.to_string())
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
            let arguments = hir::NonEmptyVec::from_vec(arguments).ok_or_else(|| {
                ImportedDefaultMaterializationError::Plan(
                    "generic dependency call has no arguments".into(),
                )
            })?;
            let application =
                self.imported_generic_applications
                    .alloc(hir::ImportedGenericCallableApplication {
                        template,
                        arguments,
                    });
            return Ok(hir::ExprKind::ImportedGenericCall {
                application,
                binding: None,
                args,
                receiver,
            });
        }
        self.imported_default_call_kind(origin, args, receiver, kind, context)
    }
}
