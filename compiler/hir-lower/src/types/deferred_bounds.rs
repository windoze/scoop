//! Bound checks whose nominal relationships are still being declared.

use super::*;

#[derive(Clone)]
pub(crate) struct DeferredTypeBound {
    argument: TypeId,
    required: TypeId,
    parameters: Vec<hir::TypeParamDecl>,
    file: usize,
    span: ast::Span,
    diagnostic: String,
    declaration: Option<crate::editing::types::TypeTarget>,
}

impl Lowerer {
    pub(crate) fn complete_parameter_bound_checks(
        &mut self,
        outer: Option<Vec<DeferredTypeBound>>,
        parameters: &[hir::TypeParamDecl],
    ) {
        let pending = self.deferred_type_bounds.as_mut().unwrap();
        for check in pending.iter_mut() {
            for parameter in &mut check.parameters {
                if let Some(complete) = parameters.iter().find(|p| p.id == parameter.id) {
                    *parameter = complete.clone();
                }
            }
        }
        if let Some(mut outer) = outer {
            outer.append(pending);
            self.deferred_type_bounds = Some(outer);
        } else {
            self.validate_deferred_type_bounds();
        }
    }

    pub(crate) fn check_type_argument_bound(
        &mut self,
        argument: TypeId,
        required: TypeId,
        span: ast::Span,
        diagnostic: String,
    ) -> bool {
        let declaration = self.current_editing_declaration();
        if let Some(pending) = &mut self.deferred_type_bounds {
            pending.push(DeferredTypeBound {
                argument,
                required,
                parameters: self.type_params_in_scope.clone(),
                file: self.current_file,
                span,
                diagnostic,
                declaration,
            });
            true
        } else if self.is_subtype(argument, required) {
            true
        } else {
            self.error(span, diagnostic);
            false
        }
    }

    pub(crate) fn validate_deferred_type_bounds(&mut self) {
        let pending = self
            .deferred_type_bounds
            .take()
            .expect("nominal declaration resolution defers bound checks");
        let file = self.current_file;
        let parameters = std::mem::take(&mut self.type_params_in_scope);
        for check in pending {
            if check
                .declaration
                .is_some_and(|target| !self.editing_type_available(target))
            {
                continue;
            }
            self.current_file = check.file;
            self.type_params_in_scope = check.parameters;
            if !self.check_type_argument_bound(
                check.argument,
                check.required,
                check.span,
                check.diagnostic,
            ) && let Some(target) = check.declaration
            {
                self.fail_editing_type_declaration(target);
            }
        }
        self.current_file = file;
        self.type_params_in_scope = parameters;
    }
}
