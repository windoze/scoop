use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(crate) fn materialize_imported_callable_body(
        &mut self,
        template: &crate::imported_generics::PreparedImportedGeneric,
    ) -> Result<hir::Body, ImportedDefaultMaterializationError> {
        let saved_locals = std::mem::replace(&mut self.locals, template.locals.clone());
        let mut context = ImportedDefaultContext {
            owner: &template.declaration,
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
            evaluation: ImportedTemplateEvaluation::Definition,
        };
        let statements = template
            .declaration
            .callable_body()
            .expect("prepared template retains its body")
            .statements()
            .iter()
            .map(|statement| self.materialize_imported_default_statement(statement, &mut context))
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
        if let hir::DefaultCallableDeclarationV1::GenericFunction(identity) = callee.declaration() {
            let declaration = self
                .dependencies
                .as_ref()
                .expect("dependency body retains its source catalog")
                .callable_declaration(scoop_identity::CallableTemplateOrigin::GenericFunction(
                    identity,
                ))
                .map_err(|error| {
                    ImportedDefaultMaterializationError::DependencySelection(error.to_string())
                })?;
            let template = self
                .request_imported_generic_template(declaration)
                .map_err(ImportedDefaultMaterializationError::Plan)?;
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
        let origin = super::super::plan::default_callable_origin(callee)
            .map_err(|error| ImportedDefaultMaterializationError::Plan(error.to_string()))?;
        self.imported_default_call_kind(origin, args, receiver, kind, context)
    }
}
