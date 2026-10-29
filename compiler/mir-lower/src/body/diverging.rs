use super::*;

impl BodyLowerer<'_> {
    pub(crate) fn lower_expr(&mut self, source: &hir::Expr) -> smir::Expr {
        let value = self.lower_expr_inner(source);
        let mut value = if matches!(value.ty, mir::Type::Class(id) if matches!(
            self.classes[id].representation,
            mir::ClassRepresentation::Intrinsic(mir::IntrinsicClassRepresentation::Nothing)
        )) && !value.is_diverging()
        {
            smir::Expr::diverging(Vec::new(), value)
        } else {
            value
        };
        let origin = self
            .module
            .debug_sources
            .expression(source.origin.definition, self.debug_records);
        value.debug_origin = self.debug_records.scoped(&origin, self.debug_scope.clone());
        value
    }
}
