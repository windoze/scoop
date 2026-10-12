use super::*;

impl Lowerer {
    pub(super) fn finish_lambda_returns(
        &mut self,
        body: &mut crate::stmt::ValueBlock,
        expected: Option<TypeId>,
        span: Span,
    ) -> Option<TypeId> {
        if let Some(expected) = expected {
            if let Some(value) = &body.value
                && !self.types_equal(expected, self.unit)
                && !self.is_subtype(value.ty, expected)
            {
                let found = self.type_name(value.ty);
                let expected = self.type_name(expected);
                self.error(
                    span,
                    format!("lambda result must be of type {expected}, found {found}"),
                );
                return None;
            }
            return Some(expected);
        }
        let mut inference = self
            .return_inference
            .take()
            .expect("lambda return inference is active");
        if inference.saw_bare {
            inference.value_types.push(self.unit);
        }
        if let Some(value) = &body.value {
            inference.value_types.push(value.ty);
        }
        let result = if inference.value_types.is_empty() {
            self.nothing_type()
        } else {
            self.least_upper_bound(&inference.value_types)
        };
        body.statements = self.adapt_inferred_returns(std::mem::take(&mut body.statements), result);
        Some(result)
    }
}
