use super::*;

impl Lowerer {
    pub(super) fn materialize_imported_callable_reference(
        &mut self,
        source: &hir::DefaultCallableReferenceV1,
        creation: hir::ExpressionOrigin,
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<hir::ExprKind, ImportedDefaultMaterializationError> {
        let ImportedTemplateEvaluation::Definition(parent) = context.evaluation else {
            return Err(ImportedDefaultMaterializationError::Plan(
                "dependency callable reference has no enclosing definition".into(),
            ));
        };
        let definition = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.callable_reference_definition(source.invoke()))
            .cloned()
            .ok_or_else(|| {
                ImportedDefaultMaterializationError::Plan(
                    "dependency callable reference is missing its invoke definition".into(),
                )
            })?;
        let target = match source.target() {
            hir::DefaultCallableReferenceTargetV1::Named(callee) => {
                hir::ImportedCallableReferenceTarget::Named(
                    self.materialize_imported_callable_target(
                        callee,
                        MemberCallKind::Ordinary,
                        context,
                    )?,
                )
            }
            hir::DefaultCallableReferenceTargetV1::Local { callee, .. } => {
                let hir::ImportedCallableTarget::Application(application) = self
                    .materialize_imported_callable_target(
                        callee,
                        MemberCallKind::Ordinary,
                        context,
                    )?
                else {
                    return Err(ImportedDefaultMaterializationError::Plan(
                        "a dependency local reference requires its lexical implementation".into(),
                    ));
                };
                hir::ImportedCallableReferenceTarget::Local(application)
            }
            hir::DefaultCallableReferenceTargetV1::BoundMember { receiver, callee } => {
                hir::ImportedCallableReferenceTarget::BoundMember {
                    receiver: Box::new(
                        self.materialize_imported_default_expression(receiver, context)?,
                    ),
                    callee: self.materialize_imported_method_callee(
                        callee,
                        creation.concrete().definition.span,
                        context,
                    )?,
                }
            }
            hir::DefaultCallableReferenceTargetV1::BoundExtension { receiver, callee } => {
                hir::ImportedCallableReferenceTarget::BoundExtension {
                    receiver: Box::new(
                        self.materialize_imported_default_expression(receiver, context)?,
                    ),
                    callee: self.materialize_imported_callable_target(
                        callee,
                        MemberCallKind::Ordinary,
                        context,
                    )?,
                }
            }
        };
        let bindings = source
            .captures()
            .iter()
            .map(|capture| self.imported_closure_capture_binding(capture.source(), parent, context))
            .collect::<Result<Vec<_>, _>>()?;
        let captures =
            self.materialize_imported_captures(source.captures(), &bindings, creation, context)?;
        let owner_type_arguments = self.imported_lexical_owner_arguments(parent);
        let function_type =
            self.materialize_imported_function_type(source.function_type(), context)?;
        Ok(hir::ExprKind::ImportedCallableReference(Box::new(
            hir::ImportedCallableReference {
                definition,
                parent,
                owner_type_arguments,
                target,
                function_type,
                captures,
                origin: creation.concrete().definition,
            },
        )))
    }
}
