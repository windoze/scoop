use super::*;

impl Lowerer {
    /// The source declaration uses Any; the selected intrinsic derives the
    /// closure's real contextual signature by removing its native context.
    pub(in crate::expr) fn foreign_callback_expected(
        &mut self,
        explicit: &[ResolvedCallTypeArgument],
        context: &ast::Expr,
        callback_source: usize,
        span: Span,
    ) -> Result<(usize, TypeId), ()> {
        let [ResolvedCallTypeArgument::Explicit { ty: native_ty, .. }] = explicit else {
            self.error(
                span,
                "`foreignCallback` requires one explicit function type".into(),
            );
            return Err(());
        };
        let hir::Type::Function(native_function_type) = self.types[*native_ty] else {
            self.error(
                span,
                "`foreignCallback` type argument must be an ordinary concrete function type".into(),
            );
            return Err(());
        };
        if !self.validate_foreign_callback_signature(native_function_type, span) {
            return Err(());
        }
        let ast::Expr::IntLiteral(literal) = context else {
            self.error(
                context.span(),
                "foreign callback `contextIndex` must be a compile-time integer literal".into(),
            );
            return Err(());
        };
        let native = self.function_types[native_function_type].clone();
        let context_index = usize::try_from(literal.magnitude).ok();
        if context_index.is_none_or(|index| index >= native.parameter_types.len()) {
            self.error(
                literal.span,
                "foreign callback `contextIndex` is outside the native signature".into(),
            );
            return Err(());
        }
        let context_index = context_index.expect("the context index is in range");
        if !matches!(self.types[native.parameter_types[context_index]], hir::Type::Ptr(pointee) if pointee == self.unit)
        {
            self.error(
                literal.span,
                "foreign callback context parameter must be exactly `Ptr<Unit>`".into(),
            );
            return Err(());
        }
        let parameters = native
            .parameter_types
            .iter()
            .enumerate()
            .filter_map(|(index, ty)| (index != context_index).then_some(*ty))
            .collect();
        let expected = self.intern_function_type(false, parameters, native.return_type);
        Ok((callback_source, expected))
    }
}
