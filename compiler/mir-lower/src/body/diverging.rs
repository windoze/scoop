use super::*;

impl BodyLowerer<'_> {
    pub(crate) fn lower_expr(&mut self, source: &hir::Expr) -> smir::Expr {
        let value = self.lower_expr_inner(source);
        if matches!(value.ty, mir::Type::Class(id) if matches!(
            self.classes[id].representation,
            mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::Nothing)
        )) && !value.is_diverging()
        {
            smir::Expr::diverging(Vec::new(), value)
        } else {
            value
        }
    }
}
