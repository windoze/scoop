//! Projection of lexical callables and their capture ABI.

use crate::{
    CallableBodyTypeArguments, CallableReferenceTarget, DefaultAnonymousFunctionV1,
    DefaultCallableBodyTypeArgumentsV1, DefaultCallableReferenceTargetV1,
    DefaultCallableReferenceV1, DefaultCaptureV1, DefaultLambdaV1, DefaultLocalFunctionV1,
};

use super::BodyProjection;

impl BodyProjection<'_, '_> {
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
            self.function_type(function.function_type)?,
            self.captures(&function.captures)?,
            owner_parameter_count(function.owner_type_param_count)?,
        )
        .map_err(super::super::DefaultBodyProjectionError::LocalFunction)
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
            owner_parameter_count(reference.owner_type_param_count)?,
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
        let selector = self.locals.binding_selector(capture.binding)?;
        let source_matches = match &capture.source.kind {
            crate::ExprKind::Local(local) => self.local(*local)? == selector,
            crate::ExprKind::Capture(binding) => *binding == capture.binding,
            _ => false,
        };
        if !source_matches {
            return Err(super::super::DefaultBodyProjectionError::InvalidCaptureSource);
        }
        let value_type = self.type_key(capture.ty)?;
        if self.type_key(capture.source.ty)? != value_type {
            return Err(super::super::DefaultBodyProjectionError::InvalidCaptureSource);
        }
        let mut origin = capture.source.origin.definition();
        origin.span = capture.first_use_span;
        Ok(DefaultCaptureV1::new(
            selector,
            value_type,
            self.origin(origin)?,
        ))
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
