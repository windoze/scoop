use super::*;

impl Lowerer {
    pub(super) fn encoding_container(
        &mut self,
        encoder: hir::Expr,
        keyed: bool,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let container = self.coding_call(
            encoder,
            if keyed { "keyed" } else { "unkeyed" },
            Vec::new(),
            span,
            sink,
        )?;
        Some(self.coding_local(container, span, sink))
    }

    pub(super) fn encode_field(
        &mut self,
        context: CodingContext,
        value: hir::Expr,
        container: hir::Expr,
        wire: Option<&str>,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<()> {
        let codec = self.select_field_codec(context, value.ty, span)?;
        // Read each data field once before obtaining its child encoder.
        let value = self.coding_local(value, span, sink);
        let (method, arguments) = match wire {
            Some(wire) => ("field", vec![self.coding_string(wire, span)]),
            None => ("element", Vec::new()),
        };
        let child = self.coding_call(container, method, arguments, span, sink)?;
        self.encode_selected_value(context, &codec, value, child, span, sink)
    }

    fn encode_selected_value(
        &mut self,
        context: CodingContext,
        selected: &SelectedCodec,
        value: hir::Expr,
        child: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<()> {
        // Codec construction or singleton initialization follows the child call.
        let child = self.coding_local(child, span, sink);
        let codec = self.selected_codec_value(context, selected, span, sink)?;
        let call = self.coding_call(codec, "encode", vec![value, child], span, sink)?;
        sink.push(hir::Statement {
            kind: hir::StatementKind::Expr(call),
            span,
        });
        Some(())
    }

    pub(in crate::class::inheritance) fn encode_tuple(
        &mut self,
        context: CodingContext,
        codecs: &[SelectedCodec],
        value: hir::Expr,
        encoder: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<()> {
        let container = self.encoding_container(encoder, false, span, sink)?;
        for (index, codec) in codecs.iter().enumerate() {
            let field = self.coding_expr(
                hir::ExprKind::FieldAccess {
                    receiver: Box::new(value.clone()),
                    field: hir::FieldRef::TupleIndex(index as u32),
                },
                codec.ty,
                span,
            );
            let field = self.coding_local(field, span, sink);
            let child = self.coding_call(container.clone(), "element", Vec::new(), span, sink)?;
            self.encode_selected_value(context, codec, field, child, span, sink)?;
        }
        self.end_coding_container(container, span, sink)
    }
}
