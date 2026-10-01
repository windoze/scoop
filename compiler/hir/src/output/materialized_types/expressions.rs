use super::*;

impl Collector<'_> {
    pub(super) fn expression(
        &mut self,
        expression: &Expr,
    ) -> Result<(), MaterializedTypeClosureError> {
        for (_, ty) in expression.type_uses() {
            self.add(ty)?;
        }
        if let ExprKind::Call { receiver, .. } = &expression.kind
            && let crate::SourceCallReceiver::Receiver { static_type } = receiver
        {
            self.add(*static_type)?;
        }
        if let ExprKind::CallableCall { function_type, .. } = &expression.kind {
            self.function_type(*function_type)?;
        }
        if let Some(ty) = expression.shared_representation_type(&self.module.types) {
            // Fixed dynamic invokes publish every concrete signature operand,
            // including private nominals nested inside tuple/function values.
            self.shared_types.insert(ty);
        }
        Ok(())
    }
}
