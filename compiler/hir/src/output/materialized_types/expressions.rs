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
                self.shared_types.insert(*static_type);
            }
        }
        if let ExprKind::CallableCall { function_type, .. } = &expression.kind {
            self.function_type(*function_type)?;
        }
        let descriptor = match expression.kind {
            ExprKind::Lambda(_)
            | ExprKind::AnonymousFunction(_)
            | ExprKind::CallableReference(_)
            | ExprKind::FunctionCoercion { .. } => Some(expression.ty),
            ExprKind::IsInstance { check_ty, .. } | ExprKind::Cast { check_ty, .. }
                if matches!(self.module.types[check_ty].kind, TypeKind::Function(_)) =>
            {
                Some(check_ty)
            }
            _ => None,
        };
        if let Some(ty) = descriptor {
            // Fixed dynamic invokes publish every concrete signature operand,
            // including private nominals nested inside tuple/function values.
            self.shared_types.insert(ty);
        }
        Ok(())
    }
}
