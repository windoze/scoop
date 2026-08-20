use super::*;

impl Lowerer {
    pub(super) fn materialize_imported_closure(
        &mut self,
        source: &hir::DefaultExpressionKindV1,
        ty: hir::TypeId,
        creation: hir::ExpressionOrigin,
        context: &ImportedDefaultContext<'_>,
    ) -> Result<hir::ExprKind, ImportedDefaultMaterializationError> {
        let (body, path, body_arguments, captures, role) = match source {
            hir::DefaultExpressionKindV1::Lambda(lambda) => (
                lambda.body(),
                lambda.definition_path(),
                lambda.body_type_arguments(),
                lambda.captures(),
                scoop_identity::LexicalCallableRole::LambdaBody,
            ),
            hir::DefaultExpressionKindV1::AnonymousFunction(function) => (
                function.body(),
                function.definition_path(),
                function.body_type_arguments(),
                function.captures(),
                scoop_identity::LexicalCallableRole::AnonymousFunctionBody,
            ),
            _ => unreachable!("closure materialization receives a lexical closure descriptor"),
        };
        let definition = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.generated_callable_definition(body))
            .ok_or_else(|| {
                ImportedDefaultMaterializationError::Plan(
                    "dependency closure is missing its original definition".into(),
                )
            })?;
        let scoop_identity::GeneratedCallableKey::Lexical { parent, .. } = definition.key() else {
            return Err(ImportedDefaultMaterializationError::Plan(
                "dependency closure body is not a lexical callable".into(),
            ));
        };
        let parent = parent.template();
        let capture_bindings = captures
            .iter()
            .map(|capture| self.materialize_capture_binding(capture, context))
            .collect::<Result<_, _>>()?;
        let template = self
            .request_imported_closure(parent, body, role, capture_bindings)
            .map_err(ImportedDefaultMaterializationError::Plan)?;
        let arguments = match body_arguments.explicit_arguments() {
            Some(arguments) => arguments
                .iter()
                .map(|argument| self.materialize_imported_default_type(argument, context))
                .collect::<Result<Vec<_>, _>>()?,
            None => context.lexical_arguments.clone(),
        };
        let hir::ImportedCallableTemplateOrigin::Closure {
            capture_bindings, ..
        } = &self.imported_generic_templates[template].declaration
        else {
            unreachable!("closure preparation retains its capture inputs")
        };
        let bindings = capture_bindings.clone();
        let captures =
            self.materialize_imported_captures(captures, &bindings, creation, context)?;
        let hir::Type::Function(function_type) = self.types[ty] else {
            return Err(ImportedDefaultMaterializationError::Plan(
                "dependency closure has a non-function type".into(),
            ));
        };
        let definition = hir::LexicalFunctionDefinition::Template(template);
        let owner_type_param_count = arguments.len();
        let body_type_arguments = hir::CallableBodyTypeArguments::Explicit(arguments);
        let span = creation.concrete().definition.span;
        Ok(match role {
            scoop_identity::LexicalCallableRole::LambdaBody => {
                hir::ExprKind::Lambda(self.lambdas.alloc(hir::Lambda {
                    definition,
                    definition_path: path.clone(),
                    function_type,
                    owner_type_param_count,
                    body_type_arguments,
                    captures,
                    span,
                }))
            }
            scoop_identity::LexicalCallableRole::AnonymousFunctionBody => {
                hir::ExprKind::AnonymousFunction(self.anonymous_functions.alloc(
                    hir::AnonymousFunction {
                        definition,
                        definition_path: path.clone(),
                        function_type,
                        owner_type_param_count,
                        body_type_arguments,
                        captures,
                        span,
                    },
                ))
            }
        })
    }

    pub(super) fn materialize_imported_function_type(
        &mut self,
        ty: &scoop_identity::SignatureTypeKey,
        context: &ImportedDefaultContext<'_>,
    ) -> Result<hir::FunctionTypeId, ImportedDefaultMaterializationError> {
        let ty = self.materialize_imported_default_type(ty, context)?;
        match self.types[ty] {
            hir::Type::Function(function) => Ok(function),
            _ => Err(ImportedDefaultMaterializationError::Plan(
                "dependency callable value has a non-function type".into(),
            )),
        }
    }

    pub(super) fn imported_closure_capture_binding(
        &self,
        source: &hir::DefaultCaptureSourceV1,
        context: &ImportedDefaultContext<'_>,
    ) -> Result<hir::BindingId, ImportedDefaultMaterializationError> {
        match source {
            hir::DefaultCaptureSourceV1::Local(selector) => context
                .local_bindings
                .get(selector)
                .copied()
                .ok_or_else(|| ImportedDefaultMaterializationError::UnknownLocal(selector.clone())),
            hir::DefaultCaptureSourceV1::EnclosingCapture(index) => context
                .captures
                .get(*index as usize)
                .copied()
                .ok_or_else(|| {
                    ImportedDefaultMaterializationError::Plan(format!(
                        "dependency body has no enclosing capture {index}"
                    ))
                }),
        }
    }

    pub(super) fn materialize_imported_captures(
        &mut self,
        captures: &[hir::DefaultCaptureV1],
        bindings: &[hir::BindingId],
        creation: hir::ExpressionOrigin,
        context: &ImportedDefaultContext<'_>,
    ) -> Result<Vec<hir::Capture>, ImportedDefaultMaterializationError> {
        if captures.len() != bindings.len() {
            return Err(ImportedDefaultMaterializationError::Plan(
                "dependency capture descriptor does not match its inputs".into(),
            ));
        }
        captures
            .iter()
            .zip(bindings.iter().copied())
            .enumerate()
            .map(|(index, (capture, binding))| {
                let ty = self.materialize_imported_default_type(capture.value_type(), context)?;
                let definition =
                    self.imported_default_definition_origin(capture.first_use_origin(), context)?;
                let mut source =
                    match capture.source() {
                        hir::DefaultCaptureSourceV1::Local(selector) => {
                            context.locals.get(selector).cloned().ok_or_else(|| {
                                ImportedDefaultMaterializationError::UnknownLocal(selector.clone())
                            })?
                        }
                        hir::DefaultCaptureSourceV1::EnclosingCapture(index) => {
                            let binding =
                                context.captures.get(*index as usize).copied().ok_or_else(
                                    || {
                                        ImportedDefaultMaterializationError::Plan(format!(
                                            "dependency body has no enclosing capture {index}"
                                        ))
                                    },
                                )?;
                            hir::Expr {
                                kind: hir::ExprKind::Capture(binding),
                                ty,
                                span: definition.span,
                                origin: hir::ExpressionOrigin::Definition(definition),
                            }
                        }
                    };
                // The value is read in the enclosing body at creation. Its
                // first lexical use remains the definition location.
                source.span = definition.span;
                source.origin =
                    hir::ExpressionOrigin::Instantiated(hir::ConcreteExpressionOrigin {
                        definition,
                        evaluation: creation.concrete().evaluation,
                    });
                Ok(hir::Capture {
                    binding,
                    name: format!("$capture.{index}"),
                    ty,
                    first_use_span: definition.span,
                    source,
                })
            })
            .collect::<Result<_, ImportedDefaultMaterializationError>>()
    }
}
