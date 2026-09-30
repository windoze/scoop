use super::*;
use crate::expr::named_calls::imported_dependency::ImportedCallableCandidate;

impl Lowerer {
    pub(super) fn materialize_imported_callable_reference(
        &mut self,
        source: &hir::DefaultCallableReferenceV1,
        creation: hir::ExpressionOrigin,
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<hir::ExprKind, ImportedDefaultMaterializationError> {
        let parent = context.parent;
        let definition = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.generated_callable_definition(source.invoke()))
            .cloned()
            .ok_or_else(|| {
                ImportedDefaultMaterializationError::Plan(
                    "dependency callable reference is missing its invoke definition".into(),
                )
            })?;
        let definition_origin = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| {
                dependencies.generated_callable_definition_origin(source.invoke())
            })
            .cloned()
            .ok_or_else(|| {
                ImportedDefaultMaterializationError::Plan(
                    "dependency callable reference is missing its invoke definition origin".into(),
                )
            })?;
        let origin = self.imported_default_definition_origin(&definition_origin, context)?;
        let target = match source.target() {
            hir::DefaultCallableReferenceTargetV1::Named(callee) => {
                hir::CallableReferenceTarget::Named(self.materialize_imported_callable_target(
                    callee,
                    MemberCallKind::Ordinary,
                    context,
                )?)
            }
            hir::DefaultCallableReferenceTargetV1::Local { callee, .. } => {
                let hir::CallableTarget::Application(application) = self
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
                let template = self.imported_generic_applications[application].template;
                let hir::ImportedCallableTemplateOrigin::Local { descriptor, .. } =
                    &self.imported_generic_templates[template].declaration
                else {
                    return Err(ImportedDefaultMaterializationError::Plan(
                        "a dependency local reference requires its local declaration".into(),
                    ));
                };
                hir::CallableReferenceTarget::Local {
                    definition_path: descriptor.definition_path().clone(),
                    callee: hir::CallableTarget::Application(application),
                }
            }
            hir::DefaultCallableReferenceTargetV1::BoundMember { receiver, callee } => {
                let receiver =
                    Box::new(self.materialize_imported_default_expression(receiver, context)?);
                if let Some((declaration, intrinsic)) = self.imported_reference_intrinsic(callee)? {
                    if intrinsic.requires_arithmetic_exception() {
                        self.prepare_arithmetic_exception_type().map_err(|error| {
                            ImportedDefaultMaterializationError::Plan(
                                error.diagnostic("integer reference exception type"),
                            )
                        })?;
                    }
                    hir::CallableReferenceTarget::BoundIntrinsic {
                        receiver,
                        declaration,
                        intrinsic,
                    }
                } else {
                    hir::CallableReferenceTarget::BoundMember {
                        receiver,
                        callee: self
                            .materialize_imported_method_callee(callee, creation, context)?,
                    }
                }
            }
            hir::DefaultCallableReferenceTargetV1::BoundExtension { receiver, callee } => {
                hir::CallableReferenceTarget::BoundExtension {
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
            .map(|capture| self.materialize_capture_binding(capture, context))
            .collect::<Result<Vec<_>, _>>()?;
        let captures =
            self.materialize_imported_captures(source.captures(), &bindings, creation, context)?;
        let owner_type_arguments = context.lexical_arguments.clone();
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
                origin,
            },
        )))
    }

    fn imported_reference_intrinsic(
        &self,
        callee: &hir::DefaultMethodCalleeV1,
    ) -> Result<
        Option<(
            scoop_identity::PersistentFunctionId,
            hir::PrimitiveMemberIntrinsic,
        )>,
        ImportedDefaultMaterializationError,
    > {
        let hir::DefaultMethodCalleeV1::Callable(callee) = callee else {
            return Ok(None);
        };
        let hir::DefaultCallableDeclarationV1::Function(id) = callee.declaration() else {
            return Ok(None);
        };
        let declaration = self
            .dependencies
            .as_ref()
            .expect("a dependency reference retains its source catalog")
            .callable_declaration(scoop_identity::CallableTemplateOrigin::Function(id))
            .map_err(|error| {
                ImportedDefaultMaterializationError::DependencySelection(error.to_string())
            })?;
        Ok(
            ImportedCallableCandidate::Declaration(Box::new(declaration))
                .normalized_intrinsic()
                .map(|intrinsic| (id, intrinsic)),
        )
    }
}
