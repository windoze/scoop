use super::*;

impl Lowerer {
    pub(in crate::class::inheritance::decoding) fn decode_selected_value(
        &mut self,
        context: CodingContext,
        selected: &SelectedCodec,
        child: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        // The child is obtained before singleton initialization or codec construction.
        let child = self.coding_local(child, span, sink);
        let codec = self.selected_codec_value(context, selected, span, sink)?;
        let value = self.coding_call(codec, "decode", vec![child], span, sink)?;
        Some(self.coding_local(value, span, sink))
    }

    pub(in crate::class::inheritance) fn decode_tuple(
        &mut self,
        context: CodingContext,
        ty: TypeId,
        codecs: &[SelectedCodec],
        decoder: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let elements = self.coding_call(decoder, "unkeyed", Vec::new(), span, sink)?;
        let elements = self.coding_local(elements, span, sink);
        let mut values = Vec::with_capacity(codecs.len());
        for codec in codecs {
            let child = self.coding_call(elements.clone(), "element", Vec::new(), span, sink)?;
            values.push(self.decode_selected_value(context, codec, child, span, sink)?);
        }
        self.end_coding_container(elements, span, sink)?;
        Some(self.coding_expr(hir::ExprKind::TupleLiteral(values), ty, span))
    }
}
