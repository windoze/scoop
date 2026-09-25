use super::*;

impl Collector<'_> {
    pub(super) fn expression(
        &mut self,
        expression: &Expr,
        meter: &mut BudgetMeter,
    ) -> Result<(), MaterializedTypeClosureError> {
        for (_, ty) in expression.type_uses() {
            self.add(ty, 1, meter)?;
        }
        if let ExprKind::Call { receiver, .. } | ExprKind::ImportedDependencyCall { receiver, .. } =
            &expression.kind
            && let crate::SourceCallReceiver::Receiver { static_type } = receiver
        {
            self.add(*static_type, 1, meter)?;
        }
        if let ExprKind::CallableCall { function_type, .. } = &expression.kind {
            self.function_type(*function_type, 1, meter)?;
        }
        Ok(())
    }
}
