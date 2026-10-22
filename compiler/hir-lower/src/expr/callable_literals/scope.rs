use super::*;
use scoop_identity::{LexicalCallableRole, StructuralDefinitionPath, StructuralDefinitionSiteRole};

pub(super) enum LiteralKind {
    Lambda,
    AnonymousFunction,
}

impl Lowerer {
    /// Complete lowering and prefix queries enter the same lexical callable.
    pub(super) fn with_callable_literal_scope<R>(
        &mut self,
        kind: LiteralKind,
        label: Option<&ast::Ident>,
        is_suspend: bool,
        has_receiver: bool,
        span: Span,
        lower: impl FnOnce(&mut Self, hir::ExpressionOrigin, &StructuralDefinitionPath) -> R,
    ) -> R {
        let capture_environment = self.capture_environment();
        // Nested callables obtain constructor parameters through their captures.
        let outer_constructor_parameters = std::mem::take(&mut self.constructor_params_in_scope);
        let literal_origin = self.expression_origin(span);
        let definition_path = self
            .definition_paths
            .next(StructuralDefinitionSiteRole::Lambda);
        let outer_definition_paths = std::mem::replace(
            &mut self.definition_paths,
            crate::definition_paths::DefinitionPathContext::nested(&definition_path),
        );
        let outer_locals = std::mem::take(&mut self.locals);
        let nested_scope = self.scopes.nested_callable();
        let outer_scopes = std::mem::replace(&mut self.scopes, nested_scope);
        let outer_return_ty = self.current_return_ty;
        let outer_return_inference = self.return_inference.take();
        let outer_callable_label = std::mem::replace(
            &mut self.current_callable_label,
            label.map(|name| name.text.clone()),
        );
        let outer_fn_name = std::mem::take(&mut self.current_fn_name);
        let outer_loop_targets = std::mem::take(&mut self.loop_targets);
        let outer_source_context = self.current_source_context;
        let outer_owner = self.current_owner;
        let outer_this = self.current_this.take();
        let outer_initialization = if has_receiver {
            self.initialization_context.take()
        } else {
            None
        };
        let outer_receivers = self.receiver_scopes.len();
        let outer_smart_casts = self.smart_casts.clone();
        self.capture_contexts.push(CaptureContext {
            available: capture_environment,
            captures: Vec::new(),
            by_binding: std::collections::HashMap::new(),
        });
        let role = match kind {
            LiteralKind::Lambda => {
                let function_number = self.next_lambda_function;
                self.next_lambda_function += 1;
                self.current_fn_name = format!("$lambda.{function_number}");
                LexicalCallableRole::LambdaBody
            }
            LiteralKind::AnonymousFunction => {
                let function_number = self.next_anonymous_function;
                self.next_anonymous_function += 1;
                self.current_fn_name = format!("$anonymous.{function_number}");
                LexicalCallableRole::AnonymousFunctionBody
            }
        };
        self.set_source_context(hir::SourceContextSubject::LexicalCallable {
            root: self.current_definition_root(),
            path: definition_path.clone(),
            role,
        });
        self.push_suspension_context(if is_suspend {
            SuspensionContext::SuspendFunction
        } else {
            SuspensionContext::Forbidden(ForbiddenSuspendContext::Function)
        });
        self.push_safety_context(hir::Safety::Safe);
        self.push_scope();
        let outer_default_template = std::mem::replace(&mut self.lowering_default_template, false);

        let lowered = lower(self, literal_origin, &definition_path);

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
        self.current_owner = outer_owner;
        self.current_this = outer_this;
        if let Some(initialization) = outer_initialization {
            self.initialization_context = Some(initialization);
        }
        self.receiver_scopes.truncate(outer_receivers);
        self.smart_casts = outer_smart_casts;
        debug_assert!(self.loop_targets.is_empty());
        self.loop_targets = outer_loop_targets;
        lowered
    }

    pub(super) fn bind_literal_receiver(
        &mut self,
        receiver_ty: Option<TypeId>,
        label: Option<&ast::Ident>,
        span: Span,
    ) -> Option<hir::Param> {
        if let Some(ty) = receiver_ty {
            Some(self.bind_callable_receiver(ty, label.map(|label| label.text.clone()), span))
        } else {
            self.push_receiver_scope(label.map(|label| label.text.clone()), None);
            None
        }
    }
}
