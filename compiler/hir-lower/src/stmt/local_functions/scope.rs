use super::*;

impl Lowerer {
    pub(super) fn with_local_function_scope<R>(
        &mut self,
        local: hir::LocalFunctionId,
        lower: impl FnOnce(&mut Self, hir::ExpressionOrigin) -> R,
    ) -> R {
        let function = self.local_functions[local].source_function();
        let signature = self.signatures[&function].clone();
        let name = self.source_function_name(function).to_owned();
        let span = self.local_functions[local].span;
        let definition_path = self.local_functions[local].definition_path.clone();
        let outer_definition_paths = std::mem::replace(
            &mut self.definition_paths,
            crate::definition_paths::DefinitionPathContext::nested(&definition_path),
        );
        self.lower_local_parameter_interface(function, &name);

        let capture_environment = self.capture_environment();
        let outer_constructor_parameters = std::mem::take(&mut self.constructor_params_in_scope);
        let declaration_origin = self.expression_origin(span);
        let outer_locals = std::mem::take(&mut self.locals);
        let nested_scope = self.scopes.nested_callable();
        let outer_scopes = std::mem::replace(&mut self.scopes, nested_scope);
        let outer_return_ty = self.current_return_ty;
        let outer_return_inference = self.return_inference.take();
        let outer_callable_label = self.current_callable_label.replace(name.clone());
        let outer_fn_name = std::mem::take(&mut self.current_fn_name);
        let outer_loop_targets = std::mem::take(&mut self.loop_targets);
        let outer_source_context = self.current_source_context;
        let outer_this = self.current_this.take();
        let outer_receivers = self.receiver_scopes.len();
        self.push_receiver_scope(Some(name.clone()), None);
        let outer_smart_casts = self.smart_casts.clone();
        self.capture_contexts.push(CaptureContext {
            available: capture_environment,
            captures: Vec::new(),
            by_binding: std::collections::HashMap::new(),
        });
        self.current_return_ty = signature.return_ty;
        self.current_fn_name = self.functions[function].name.clone();
        self.set_source_context(hir::SourceContextSubject::Function(function));
        self.push_suspension_context(if signature.is_suspend {
            SuspensionContext::SuspendFunction
        } else {
            SuspensionContext::Forbidden(ForbiddenSuspendContext::Function)
        });
        self.push_safety_context(signature.attributes.safety);
        self.push_scope();
        let outer_default_template = std::mem::replace(&mut self.lowering_default_template, false);

        let lowered = lower(self, declaration_origin);
        self.lowering_default_template = outer_default_template;

        self.pop_scope();
        self.pop_safety_context();
        self.pop_suspension_context();
        self.capture_contexts.pop();
        self.constructor_params_in_scope = outer_constructor_parameters;
        self.locals = outer_locals;
        self.scopes = outer_scopes;
        self.current_return_ty = outer_return_ty;
        self.return_inference = outer_return_inference;
        self.current_callable_label = outer_callable_label;
        self.current_fn_name = outer_fn_name;
        self.current_source_context = outer_source_context;
        self.definition_paths = outer_definition_paths;
        self.current_this = outer_this;
        self.receiver_scopes.truncate(outer_receivers);
        self.smart_casts = outer_smart_casts;
        debug_assert!(self.loop_targets.is_empty());
        self.loop_targets = outer_loop_targets;

        lowered
    }

    pub(super) fn bind_local_parameters(&mut self, function: hir::FunctionId) -> Vec<hir::Param> {
        let sig_params = self.signatures[&function].params.clone();
        let mut params = Vec::with_capacity(sig_params.len());
        for (index, param) in sig_params.iter().enumerate() {
            if self.scopes.is_declared_here(&param.name.text) {
                self.error(
                    param.name.span,
                    format!("duplicate parameter `{}`", param.name.text),
                );
                continue;
            }
            let local = self.alloc_parameter_local(
                param.name.text.clone(),
                param.ty,
                index,
                param.name.span,
            );
            self.scopes.declare(param.name.text.clone(), local);
            params.push(hir::Param {
                name: param.name.text.clone(),
                ty: param.ty,
                local,
            });
        }
        params
    }
}
