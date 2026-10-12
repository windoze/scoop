use super::*;

impl Lowerer {
    pub(super) fn lower_local_function_body(
        &mut self,
        function: FunctionId,
        declaration: &ast::FunctionDecl,
        declared_result: TypeId,
    ) -> Vec<hir::Statement> {
        let name = &declaration.name.text;
        match &declaration.body {
            ast::FunctionBody::Block(block) => {
                let before = self.diagnostics.len();
                let statements = self.lower_block(block);
                if !self.types_equal(declared_result, self.unit)
                    && self.diagnostics.len() == before
                    && self
                        .statements_control_outcomes(&statements)
                        .can_fall_through()
                {
                    self.error(block.span, format!("non-Unit local function `{name}` may complete without returning a value"));
                }
                statements
            }
            ast::FunctionBody::Expr(expression) => {
                let mut statements = Vec::new();
                let inferring = crate::declaration_inference::infers_result(declaration);
                if !self.reject_expression_body_return(expression)
                    && let Some(value) = self.lower_expr(
                        expression,
                        &mut statements,
                        (!inferring).then_some(declared_result),
                    )
                {
                    let result = if inferring {
                        self.set_inferred_function_result(function, value.ty);
                        value.ty
                    } else {
                        declared_result
                    };
                    if self.is_subtype(value.ty, result) {
                        let value = self.adapt_to(value, result);
                        self.push_return(Some(value), expression.span(), &mut statements);
                    } else {
                        let message = self.with_nominal_invariance_detail(
                            format!(
                                "body of local function `{name}` must be of type {}, found {}",
                                self.type_name(result),
                                self.type_name(value.ty)
                            ),
                            value.ty,
                            result,
                        );
                        self.error(expression.span(), message);
                    }
                }
                statements
            }
            ast::FunctionBody::None => {
                self.error(
                    declaration.name.span,
                    format!("local function `{name}` must have a body"),
                );
                Vec::new()
            }
        }
    }
}
