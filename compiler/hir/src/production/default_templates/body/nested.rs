//! Projection of lexical callables and their capture ABI.

use crate::{
    CallableBodyTypeArguments, CallableReferenceTarget, DefaultAnonymousFunctionV1,
    DefaultCallableBodyTypeArgumentsV1, DefaultCallableReferenceTargetV1,
    DefaultCallableReferenceV1, DefaultCaptureV1, DefaultLambdaV1, DefaultLocalFunctionV1,
};

use super::BodyProjection;

impl BodyProjection<'_, '_> {
    pub(super) fn imported_callable_reference(
        &mut self,
        reference: &crate::ImportedCallableReference,
    ) -> Result<DefaultCallableReferenceV1, super::super::DefaultBodyProjectionError> {
        let target = self.imported_reference_target(&reference.target)?;
        let scoop_identity::GeneratedCallableKey::CallableReferenceInvoke { path, .. } =
            reference.definition.key()
        else {
            unreachable!("an imported reference retains its invoke definition");
        };
        DefaultCallableReferenceV1::try_new(
            reference.definition.id(),
            path.clone(),
            target,
            self.function_type(reference.function_type)?,
            self.captures(&reference.captures)?,
            owner_parameter_count(reference.owner_type_arguments.len())?,
        )
        .map_err(super::super::DefaultBodyProjectionError::CallableReference)
    }

    fn imported_reference_target(
        &mut self,
        target: &crate::ImportedCallableReferenceTarget,
    ) -> Result<DefaultCallableReferenceTargetV1, super::super::DefaultBodyProjectionError> {
        Ok(match target {
            crate::ImportedCallableReferenceTarget::BoundIntrinsic {
                receiver,
                declaration,
                ..
            } => DefaultCallableReferenceTargetV1::BoundMember {
                receiver: Box::new(self.expression(receiver)?),
                callee: crate::DefaultMethodCalleeV1::Callable(
                    crate::DefaultCallableRefV1::try_new(
                        crate::DefaultCallableDeclarationV1::Function(*declaration),
                        scoop_identity::OptionalSignatureType::Absent,
                        Vec::new(),
                    )
                    .map_err(super::super::DefaultEntityProjectionError::Callable)?,
                ),
            },
            crate::ImportedCallableReferenceTarget::Named(callee) => {
                DefaultCallableReferenceTargetV1::Named(self.callable_target(*callee)?)
            }
            crate::ImportedCallableReferenceTarget::Local(application) => {
                let export = self.entities.export();
                let template = &export.imported_generic_templates
                    [export.imported_generic_applications[*application].template];
                let crate::ImportedCallableTemplateOrigin::Local { descriptor, .. } =
                    &template.declaration
                else {
                    unreachable!("a local reference retains its declaration");
                };
                DefaultCallableReferenceTargetV1::Local {
                    declaration: descriptor.declaration(),
                    callee: self
                        .entities
                        .imported_generic_callable(*application, self.binders)?,
                }
            }
            crate::ImportedCallableReferenceTarget::BoundMember { receiver, callee } => {
                DefaultCallableReferenceTargetV1::BoundMember {
                    receiver: Box::new(self.expression(receiver)?),
                    callee: self.method_callee(*callee)?,
                }
            }
            crate::ImportedCallableReferenceTarget::BoundExtension { receiver, callee } => {
                DefaultCallableReferenceTargetV1::BoundExtension {
                    receiver: Box::new(self.expression(receiver)?),
                    callee: self.callable_target(*callee)?,
                }
            }
        })
    }

    pub(super) fn callable_target(
        &self,
        callee: crate::CallableTarget,
    ) -> Result<crate::DefaultCallableRefV1, super::super::DefaultBodyProjectionError> {
        Ok(self.entities.callable_target(callee, self.binders)?)
    }

    pub(super) fn imported_closure(
        &mut self,
        closure: &crate::ImportedClosure,
    ) -> Result<crate::DefaultExpressionKindV1, super::super::DefaultBodyProjectionError> {
        let export = self.entities.export();
        let application = &export.imported_generic_applications[closure.application];
        let template = &export.imported_generic_templates[application.template];
        let crate::ImportedCallableTemplateOrigin::Closure { body, .. } = template.declaration
        else {
            unreachable!("an imported closure references its generated body")
        };
        let arguments = application.arguments.substitution(
            &export.types,
            &export.enum_applications,
            &export.struct_applications,
            &export.class_applications,
            &export.interface_applications,
        );
        let count = owner_parameter_count(arguments.len())?;
        let arguments = arguments
            .into_iter()
            .map(|ty| self.type_key(ty))
            .collect::<Result<_, _>>()?;
        let arguments = DefaultCallableBodyTypeArgumentsV1::try_explicit(arguments)
            .map_err(super::super::DefaultBodyProjectionError::BodyTypeArguments)?;
        let function_type = self.function_type(closure.function_type)?;
        let captures = self.captures(&closure.captures)?;
        match closure.kind {
            crate::ImportedClosureKind::Lambda => DefaultLambdaV1::try_new(
                body,
                closure.definition_path.clone(),
                function_type,
                arguments,
                captures,
                count,
            )
            .map(crate::DefaultExpressionKindV1::Lambda),
            crate::ImportedClosureKind::AnonymousFunction => DefaultAnonymousFunctionV1::try_new(
                body,
                closure.definition_path.clone(),
                function_type,
                arguments,
                captures,
                count,
            )
            .map(crate::DefaultExpressionKindV1::AnonymousFunction),
        }
        .map_err(super::super::DefaultBodyProjectionError::LexicalCallable)
    }

    pub(super) fn lambda(
        &mut self,
        id: crate::LambdaId,
    ) -> Result<DefaultLambdaV1, super::super::DefaultBodyProjectionError> {
        let lambda = super::super::arena_get(&self.entities.export().lambdas, id).ok_or(
            super::super::DefaultEntityProjectionError::Unknown {
                kind: "lambda",
                index: super::super::raw_index(id),
            },
        )?;

        DefaultLambdaV1::try_new(
            self.entities.generated_function_id(lambda.function)?,
            lambda.definition_path.clone(),
            self.function_type(lambda.function_type)?,
            self.body_type_arguments(&lambda.body_type_arguments)?,
            self.captures(&lambda.captures)?,
            owner_parameter_count(lambda.owner_type_param_count)?,
        )
        .map_err(super::super::DefaultBodyProjectionError::LexicalCallable)
    }

    pub(super) fn anonymous_function(
        &mut self,
        id: crate::AnonymousFunctionId,
    ) -> Result<DefaultAnonymousFunctionV1, super::super::DefaultBodyProjectionError> {
        let function = super::super::arena_get(&self.entities.export().anonymous_functions, id)
            .ok_or(super::super::DefaultEntityProjectionError::Unknown {
                kind: "anonymous function",
                index: super::super::raw_index(id),
            })?;

        DefaultAnonymousFunctionV1::try_new(
            self.entities.generated_function_id(function.function)?,
            function.definition_path.clone(),
            self.function_type(function.function_type)?,
            self.body_type_arguments(&function.body_type_arguments)?,
            self.captures(&function.captures)?,
            owner_parameter_count(function.owner_type_param_count)?,
        )
        .map_err(super::super::DefaultBodyProjectionError::LexicalCallable)
    }

    pub(super) fn local_function(
        &mut self,
        id: crate::LocalFunctionId,
    ) -> Result<DefaultLocalFunctionV1, super::super::DefaultBodyProjectionError> {
        let function = self.local_function_record(id)?;

        DefaultLocalFunctionV1::try_new(
            self.entities
                .source_callable_declaration(function.function)?,
            function.definition_path.clone(),
            self.entities.local_function_signature(id, self.binders)?,
            self.captures(&function.captures)?,
            owner_parameter_count(function.owner_type_arguments.len())?,
        )
        .map_err(super::super::DefaultBodyProjectionError::LocalFunction)
    }

    pub(super) fn local_function_origin(
        &self,
        id: crate::LocalFunctionId,
    ) -> Result<crate::ExportDefinitionSourceV1, super::super::DefaultBodyProjectionError> {
        self.origin(self.local_function_record(id)?.origin)
    }

    pub(super) fn local_function_declaration(
        &self,
        id: crate::LocalFunctionId,
    ) -> Result<scoop_identity::CallableTemplateOrigin, super::super::DefaultBodyProjectionError>
    {
        let function = self.local_function_record(id)?;
        self.entities
            .source_callable_declaration(function.function)
            .map_err(Into::into)
    }

    pub(super) fn callable_reference(
        &mut self,
        id: crate::CallableReferenceId,
    ) -> Result<DefaultCallableReferenceV1, super::super::DefaultBodyProjectionError> {
        let reference = super::super::arena_get(&self.entities.export().callable_references, id)
            .ok_or(super::super::DefaultEntityProjectionError::Unknown {
                kind: "callable reference",
                index: super::super::raw_index(id),
            })?;
        let target = match &reference.target {
            CallableReferenceTarget::Imported(target) => self.imported_reference_target(target)?,
            CallableReferenceTarget::Named(callee) => DefaultCallableReferenceTargetV1::Named(
                self.entities.callable(*callee, self.binders)?,
            ),
            CallableReferenceTarget::Local {
                local_function,
                callee,
            } => DefaultCallableReferenceTargetV1::Local {
                declaration: self.local_function_declaration(*local_function)?,
                callee: self.entities.callable(*callee, self.binders)?,
            },
            CallableReferenceTarget::BoundMember { receiver, callee } => {
                DefaultCallableReferenceTargetV1::BoundMember {
                    receiver: Box::new(self.expression(receiver)?),
                    callee: self.method_callee(*callee)?,
                }
            }
            CallableReferenceTarget::BoundExtension { receiver, callee } => {
                DefaultCallableReferenceTargetV1::BoundExtension {
                    receiver: Box::new(self.expression(receiver)?),
                    callee: self.entities.callable(*callee, self.binders)?,
                }
            }
        };

        DefaultCallableReferenceV1::try_new(
            self.entities
                .callable_reference_invoke(reference.definition_root, &reference.definition_path)?,
            reference.definition_path.clone(),
            target,
            self.function_type(reference.function_type)?,
            self.captures(&reference.captures)?,
            owner_parameter_count(reference.owner_type_arguments.len())?,
        )
        .map_err(super::super::DefaultBodyProjectionError::CallableReference)
    }

    fn body_type_arguments(
        &self,
        arguments: &CallableBodyTypeArguments,
    ) -> Result<DefaultCallableBodyTypeArgumentsV1, super::super::DefaultBodyProjectionError> {
        match arguments {
            CallableBodyTypeArguments::Lexical => Ok(DefaultCallableBodyTypeArgumentsV1::lexical()),
            CallableBodyTypeArguments::Explicit(arguments) => {
                let arguments = arguments
                    .iter()
                    .map(|&argument| self.type_key(argument))
                    .collect::<Result<Vec<_>, _>>()?;
                DefaultCallableBodyTypeArgumentsV1::try_explicit(arguments)
                    .map_err(super::super::DefaultBodyProjectionError::BodyTypeArguments)
            }
        }
    }

    fn captures(
        &self,
        captures: &[crate::Capture],
    ) -> Result<Vec<DefaultCaptureV1>, super::super::DefaultBodyProjectionError> {
        captures
            .iter()
            .map(|capture| self.capture(capture))
            .collect()
    }

    fn capture(
        &self,
        capture: &crate::Capture,
    ) -> Result<DefaultCaptureV1, super::super::DefaultBodyProjectionError> {
        let source = match &capture.source.kind {
            crate::ExprKind::Local(local) => {
                crate::DefaultCaptureSourceV1::Local(self.local(*local)?)
            }
            crate::ExprKind::ConstructorParam(parameter) => {
                crate::DefaultCaptureSourceV1::Local(self.locals.constructor_parameter(*parameter)?)
            }
            crate::ExprKind::Capture(binding) if *binding == capture.binding => {
                self.locals.capture_source(*binding)?
            }
            _ => return Err(super::super::DefaultBodyProjectionError::InvalidCaptureSource),
        };
        let value_type = self.type_key(capture.ty)?;
        if self.type_key(capture.source.ty)? != value_type {
            return Err(super::super::DefaultBodyProjectionError::InvalidCaptureSource);
        }
        let mut origin = capture.source.origin.definition();
        origin.span = capture.first_use_span;
        let origin = self.origin(origin)?;
        let binding = self.capture_binding(capture.binding)?;
        let capture = match source {
            crate::DefaultCaptureSourceV1::Local(selector) => {
                DefaultCaptureV1::new(selector, value_type, origin)
            }
            crate::DefaultCaptureSourceV1::EnclosingCapture(index) => {
                DefaultCaptureV1::from_enclosing_capture(index, value_type, origin)
            }
        };
        Ok(capture.with_binding(binding))
    }

    fn capture_binding(
        &self,
        binding: crate::BindingId,
    ) -> Result<crate::DefaultCaptureBindingV1, super::super::DefaultBodyProjectionError> {
        for (_, scope) in self.entities.export().default_local_value_scopes.iter() {
            let Some(value) = scope.values.iter().find(|value| value.binding == binding) else {
                continue;
            };
            let definition = match value.definition {
                crate::LocalValueDefinitionSite::Source(origin) => {
                    crate::TemplateLocalDefinitionV1::Source(self.origin(origin)?)
                }
                crate::LocalValueDefinitionSite::Synthetic => {
                    crate::TemplateLocalDefinitionV1::Synthetic
                }
            };
            return Ok(crate::DefaultCaptureBindingV1::Definition {
                owner: scope.definition_root,
                scope: scope.definition_path.clone(),
                selector: value.selector.clone(),
                definition,
            });
        }
        Ok(crate::DefaultCaptureBindingV1::Source)
    }

    fn local_function_record(
        &self,
        id: crate::LocalFunctionId,
    ) -> Result<&crate::LocalFunction, super::super::DefaultBodyProjectionError> {
        super::super::arena_get(&self.entities.export().local_functions, id).ok_or_else(|| {
            super::super::DefaultEntityProjectionError::Unknown {
                kind: "local function",
                index: super::super::raw_index(id),
            }
            .into()
        })
    }
}

fn owner_parameter_count(count: usize) -> Result<u32, super::super::DefaultBodyProjectionError> {
    u32::try_from(count)
        .map_err(|_| super::super::DefaultBodyProjectionError::TooManyOwnerTypeParameters)
}
