use super::*;

impl Collector<'_> {
    pub(super) fn expression(
        &mut self,
        expression: &Expr,
    ) -> Result<(), MaterializedTypeClosureError> {
        for (_, ty) in expression.type_uses() {
            self.add(ty)?;
        }
        if let ExprKind::Call {
            callee, receiver, ..
        } = &expression.kind
            && let crate::SourceCallReceiver::Receiver { static_type } = receiver
        {
            self.add(*static_type)?;
            if matches!(callee, CallableTarget::Imported(_)) {
                self.dependency_receivers.insert(*static_type);
            }
        }
        if let ExprKind::CallableCall { function_type, .. } = &expression.kind {
            self.function_type(*function_type)?;
        }
        Ok(())
    }
}
