use super::*;

impl Lowerer {
    pub(crate) fn compiler_input_string(&mut self, source: &ast::Expr) -> Option<String> {
        let mut probe = self.clone();
        let value = probe.evaluate_const_expression(
            source,
            Some(self.string),
            self.current_file,
            &[],
            &[],
            &mut [],
            &mut Vec::new(),
        )?;
        let hir::HirConstantImage::String(value) = value.value else {
            return None;
        };
        *self = probe;
        Some(value)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn evaluate_const_input_call(
        &mut self,
        call: &ast::CallExpr,
        expected: Option<hir::TypeId>,
        file: usize,
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
        states: &mut [ConstState],
        stack: &mut Vec<usize>,
    ) -> Option<EvaluatedConst> {
        let mut call = call.clone();
        // Pending const properties are evaluated through the existing worklist
        // before ordinary overload resolution, preserving names and locations.
        for argument in &mut call.args {
            let value = self.evaluate_const_expression(
                &argument.expression,
                None,
                file,
                declarations,
                ordinary,
                states,
                stack,
            )?;
            if let hir::ConstPropertyValue::String(value) = value.value {
                argument.expression = ast::Expr::StringLiteral {
                    value,
                    span: argument.expression.span(),
                };
            }
        }
        let previous = self
            .constant_call_context
            .replace(crate::expr::ConstantCallContext {
                file: self.current_file,
                span: call.span,
                intrinsic: None,
            });
        let mut statements = Vec::new();
        let expression = self.lower_expr(&ast::Expr::Call(call.clone()), &mut statements, expected);
        let context = std::mem::replace(&mut self.constant_call_context, previous)
            .expect("constant call resolution retains its context");
        let expression = expression?;
        if !matches!(
            context.intrinsic,
            Some(hir::IntrinsicFunctionKind::CompileEnv | hir::IntrinsicFunctionKind::IncludeStr)
        ) || !statements.is_empty()
        {
            self.error(call.span, "const initializer calls must select a compiler input intrinsic; ordinary functions and constructors are not constant expressions".into());
            return None;
        }
        let value = self
            .expression_constant(&expression)
            .expect("compiler input intrinsics produce constant values");
        Some(EvaluatedConst {
            value,
            ty: expression.ty,
        })
    }
}
