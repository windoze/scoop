use super::*;

impl Lowerer {
    pub(super) fn instantiate_default_local_function(
        &mut self,
        source: hir::LocalFunctionId,
        context: &mut InstantiationContext,
    ) -> hir::LocalFunctionId {
        let mut function = self.local_functions[source].clone();
        let own_parameters = self.functions[function.function]
            .type_params()
            .into_iter()
            .skip(function.owner_type_param_count)
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>();
        let mut bindings = context.bindings.clone();
        bindings.extend(
            own_parameters
                .into_iter()
                .map(|parameter| (parameter, self.intern_type(Type::Param(parameter)))),
        );
        let signature = self.intern_type(Type::Function(function.function_type));
        let signature = self.instantiate_method_ty(signature, &bindings);
        let Type::Function(function_type) = self.types[signature] else {
            unreachable!("local function signature substitution preserves its kind")
        };
        function.function_type = function_type;
        function.captures = function
            .captures
            .iter()
            .map(|capture| self.instantiate_default_capture(capture, context))
            .collect();
        let target = self.local_functions.alloc(function);
        assert!(
            context.local_functions.insert(source, target).is_none(),
            "a default expansion declares each local function once"
        );
        target
    }

    pub(super) fn instantiate_default_lambda(
        &mut self,
        source: hir::LambdaId,
        context: &mut InstantiationContext,
    ) -> hir::LambdaId {
        let source = self.lambdas[source].clone();
        let body_type_arguments = self.instantiate_callable_body_arguments(
            source.function,
            &source.body_type_arguments,
            context,
        );
        let function_type = self.instantiate_default_function_type(source.function_type, context);
        let captures = source
            .captures
            .iter()
            .map(|capture| self.instantiate_default_capture(capture, context))
            .collect();
        self.lambdas.alloc(hir::Lambda {
            definition_root: source.definition_root,
            definition_path: source.definition_path,
            function: source.function,
            function_type,
            owner_type_param_count: source.owner_type_param_count,
            body_type_arguments,
            captures,
            span: source.span,
        })
    }

    pub(super) fn instantiate_default_anonymous(
        &mut self,
        source: hir::AnonymousFunctionId,
        context: &mut InstantiationContext,
    ) -> hir::AnonymousFunctionId {
        let source = self.anonymous_functions[source].clone();
        let body_type_arguments = self.instantiate_callable_body_arguments(
            source.function,
            &source.body_type_arguments,
            context,
        );
        let function_type = self.instantiate_default_function_type(source.function_type, context);
        let captures = source
            .captures
            .iter()
            .map(|capture| self.instantiate_default_capture(capture, context))
            .collect();
        self.anonymous_functions.alloc(hir::AnonymousFunction {
            definition_root: source.definition_root,
            definition_path: source.definition_path,
            function: source.function,
            function_type,
            owner_type_param_count: source.owner_type_param_count,
            body_type_arguments,
            captures,
            span: source.span,
        })
    }

    pub(super) fn instantiate_default_reference(
        &mut self,
        source: hir::CallableReferenceId,
        context: &mut InstantiationContext,
    ) -> hir::CallableReferenceId {
        let source = self.callable_references[source].clone();
        let target = match source.target {
            hir::CallableReferenceTarget::Named(callee) => hir::CallableReferenceTarget::Named(
                self.instantiate_default_callable(callee, context),
            ),
            hir::CallableReferenceTarget::Local {
                local_function,
                callee,
            } => hir::CallableReferenceTarget::Local {
                local_function: context.local_function(local_function),
                callee: self.instantiate_default_callable(callee, context),
            },
            hir::CallableReferenceTarget::BoundMember { receiver, callee } => {
                hir::CallableReferenceTarget::BoundMember {
                    receiver: Box::new(self.instantiate_default_expr(&receiver, context)),
                    callee: self.instantiate_default_method_callee(callee, context),
                }
            }
            hir::CallableReferenceTarget::BoundExtension { receiver, callee } => {
                hir::CallableReferenceTarget::BoundExtension {
                    receiver: Box::new(self.instantiate_default_expr(&receiver, context)),
                    callee: self.instantiate_default_callable(callee, context),
                }
            }
        };
        let function_type = self.instantiate_default_function_type(source.function_type, context);
        let captures = source
            .captures
            .iter()
            .map(|capture| self.instantiate_default_capture(capture, context))
            .collect();
        let owner_type_arguments = source
            .owner_type_arguments
            .into_iter()
            .map(|argument| self.instantiate_method_ty(argument, &context.bindings))
            .collect();
        self.callable_references.alloc(hir::CallableReference {
            definition_root: source.definition_root,
            definition_path: source.definition_path,
            target,
            function_type,
            owner_type_arguments,
            captures,
            origin: source.origin,
            span: source.span,
        })
    }

    pub(super) fn instantiate_default_capture(
        &mut self,
        source: &hir::Capture,
        context: &mut InstantiationContext,
    ) -> hir::Capture {
        hir::Capture {
            binding: source.binding,
            name: source.name.clone(),
            ty: self.instantiate_method_ty(source.ty, &context.bindings),
            first_use_span: source.first_use_span,
            source: self.instantiate_default_expr(&source.source, context),
        }
    }

    fn instantiate_callable_body_arguments(
        &mut self,
        function: hir::FunctionId,
        source: &hir::CallableBodyTypeArguments,
        context: &InstantiationContext,
    ) -> hir::CallableBodyTypeArguments {
        let arguments = match source {
            hir::CallableBodyTypeArguments::Lexical => self.functions[function]
                .type_params()
                .into_iter()
                .map(|parameter| parameter.id)
                .collect::<Vec<_>>()
                .into_iter()
                .map(|parameter| self.intern_type(Type::Param(parameter)))
                .collect(),
            hir::CallableBodyTypeArguments::Explicit(arguments) => arguments.clone(),
        };
        hir::CallableBodyTypeArguments::Explicit(
            arguments
                .into_iter()
                .map(|argument| self.instantiate_method_ty(argument, &context.bindings))
                .collect(),
        )
    }
}
