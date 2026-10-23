use super::*;

mod body;
mod header;
mod patching;
mod recovered;
mod scope;

use patching::patch_local_function_calls;

impl Lowerer {
    pub(super) fn lower_local_function_decl(
        &mut self,
        decl: &ast::FunctionDecl,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::LocalFunctionId> {
        let outer_type_params = self.type_params_in_scope.clone();
        let prepared = (|| {
            let local = self.prepare_local_function(decl.header())?;
            let function = self.local_functions[local].source_function();
            let return_ty = self.signatures[&function].return_ty;
            let inference_errors_before = self.diagnostics.len();
            self.begin_local_result_inference(function, decl);
            let lowered = self.with_local_function_scope(local, |state, declaration_origin| {
                let params = state.bind_local_parameters(function);
                let mut statements = state.lower_context_entry(function);
                let body_statements = state.lower_local_function_body(function, decl, return_ty);
                statements.extend(body_statements);
                let captures = state.finish_current_captures(declaration_origin);
                patch_local_function_calls(state, &mut statements, local, &captures);
                let mut abi_params = Vec::with_capacity(captures.len() + params.len());
                for capture in &captures {
                    let capture_local = state.alloc_synthetic_local(
                        format!("$capture.{}", capture.name),
                        capture.ty,
                        false,
                        scoop_identity::SyntheticLocalRole::Temporary,
                    );
                    abi_params.push(hir::Param {
                        name: format!("$capture.{}", capture.name),
                        ty: capture.ty,
                        local: capture_local,
                    });
                }
                abi_params.extend(params);
                let body_locals = std::mem::take(&mut state.locals).into_arena();
                (statements, captures, abi_params, body_locals)
            });
            Some((local, inference_errors_before, lowered))
        })();
        self.type_params_in_scope = outer_type_params;
        let (local, inference_errors_before, lowered) = prepared?;
        let (statements, mut captures, params, body_locals) = lowered;
        let function = self.local_functions[local].source_function();
        self.materialize_capture_sources(&mut captures, sink);
        self.functions[function].params = params;
        self.functions[function].kind = hir::FunctionKind::User(hir::Body {
            locals: body_locals,
            statements,
        });
        self.local_functions[local].captures = captures;
        if crate::declaration_inference::infers_result(decl) {
            let result = self.signatures[&function].return_ty;
            self.check_local_type_result(result, decl.name.span, &self.scopes.clone(), true);
            let state_parameters = self.signatures[&function]
                .params
                .iter()
                .map(|parameter| parameter.ty)
                .collect();
            let function_type =
                self.intern_function_type(decl.is_suspend, state_parameters, result);
            self.local_functions[local].declaration_function_type = function_type;
            self.local_functions[local].function_type = function_type;
            self.finish_local_result_inference(
                function,
                self.diagnostics.len() == inference_errors_before,
            );
        }
        Some(local)
    }
}
