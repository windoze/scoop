use super::*;

impl BodyLowerer<'_> {
    /// Path facts change a read's view, never its local or capture storage.
    pub(super) fn lower_known_read(
        &mut self,
        source: smir::Expr,
        ty: mir::Type,
        span: Span,
    ) -> smir::Expr {
        if source.ty == ty {
            source
        } else if let mir::Type::Function(function_type) = ty {
            self.adapt_checked_function_value(source, function_type, span)
        } else {
            smir::Expr::new(
                ty.clone(),
                smir::ExprKind::Retype {
                    operand: Box::new(source),
                    ty: Box::new(ty),
                },
            )
        }
    }
}
