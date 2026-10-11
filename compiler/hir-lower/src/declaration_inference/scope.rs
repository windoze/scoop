use super::*;
use crate::scope::{LocalFunctionScopes, Scopes};

impl Lowerer {
    pub(super) fn lower_inferred_function_body(
        &mut self,
        id: FunctionId,
        source: &ast::FunctionDecl,
    ) -> hir::Body {
        self.with_declaration_inference_scope(self.function_files[&id], |this| {
            this.lower_body(id, source)
        })
    }

    pub(super) fn with_declaration_inference_scope<T>(
        &mut self,
        source_file: usize,
        operation: impl FnOnce(&mut Self) -> T,
    ) -> T {
        // A declaration query can start inside an argument, lambda, default,
        // or initializer. None of that caller's lexical state enters its body.
        let locals = std::mem::take(&mut self.locals);
        let scopes = std::mem::replace(&mut self.scopes, Scopes::new());
        let functions =
            std::mem::replace(&mut self.local_function_scopes, LocalFunctionScopes::new());
        let captures = std::mem::take(&mut self.capture_contexts);
        let parameters = std::mem::take(&mut self.type_params_in_scope);
        let constructor_parameters = std::mem::take(&mut self.constructor_params_in_scope);
        let initialization = self.initialization_context.take();
        let initialization_unit = self.current_initialization_unit.take();
        let backing = self.backing_field_context.take();
        let release = self.current_release.take();
        let return_inference = self.return_inference.take();
        let uninitialized = std::mem::take(&mut self.uninitialized_condition_bindings);
        let smart_casts = std::mem::take(&mut self.smart_casts);
        let place_inputs = self.place_index_inputs.take();
        let derived_origin = self.derived_expression_origin.take();
        let default_template = std::mem::replace(&mut self.lowering_default_template, false);
        let file = std::mem::replace(&mut self.current_file, source_file);
        let name = std::mem::take(&mut self.current_fn_name);
        let return_ty = self.current_return_ty;
        let owner = self.current_owner;
        let this = self.current_this;
        let result = operation(self);
        self.locals = locals;
        self.scopes = scopes;
        self.local_function_scopes = functions;
        self.capture_contexts = captures;
        self.type_params_in_scope = parameters;
        self.constructor_params_in_scope = constructor_parameters;
        self.initialization_context = initialization;
        self.current_initialization_unit = initialization_unit;
        self.backing_field_context = backing;
        self.current_release = release;
        self.return_inference = return_inference;
        self.uninitialized_condition_bindings = uninitialized;
        self.smart_casts = smart_casts;
        self.place_index_inputs = place_inputs;
        self.derived_expression_origin = derived_origin;
        self.lowering_default_template = default_template;
        self.current_file = file;
        self.current_fn_name = name;
        self.current_return_ty = return_ty;
        self.current_owner = owner;
        self.current_this = this;
        result
    }
}
