use super::*;

impl Lowerer {
    pub(super) fn coding_tuple_function(
        &mut self,
        context: CodingContext,
        target: TypeId,
        elements: &[SelectedCodec],
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let stream = self.core_coding_nominal(context.direction.stream())?;
        let stream = self
            .apply_nominal_type(stream, Vec::new())
            .expect("a coding stream is an ordinary interface");
        let (parameters, result) = match context.direction {
            CodingDirection::Encode => (vec![target, stream], self.unit),
            CodingDirection::Decode => (vec![stream], target),
        };
        let function =
            self.lower_synthesized_lambda(&parameters, result, span, |state, parameters| {
                let mut statements = Vec::new();
                let value = match context.direction {
                    CodingDirection::Encode => {
                        state.encode_tuple(
                            context,
                            elements,
                            parameters[0].clone(),
                            parameters[1].clone(),
                            span,
                            &mut statements,
                        )?;
                        state.coding_expr(hir::ExprKind::UnitLiteral, state.unit, span)
                    }
                    CodingDirection::Decode => state.decode_tuple(
                        context,
                        target,
                        elements,
                        parameters[0].clone(),
                        span,
                        &mut statements,
                    )?,
                };
                Some(crate::stmt::ValueBlock {
                    statements,
                    value: Some(value),
                })
            })?;
        let adapter = self.core_coding_nominal(context.direction.function_codec())?;
        let adapter = self
            .apply_nominal_type(adapter, vec![target])
            .expect("a function codec has one unconstrained parameter");
        let record = self.coding_primary(adapter, span)?;
        self.call_coding_constructor(adapter, &record, vec![function], span, sink)
    }
}
