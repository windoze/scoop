use super::*;
use crate::NominalTarget;

impl Lowerer {
    fn selected_decoder_value(
        &mut self,
        context: DecodeContext,
        selected: &SelectedDecoder,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let value = match &selected.value {
            DecoderValue::Dependency(dependency) => {
                self.decoding_dependency_value(dependency, span)?
            }
            DecoderValue::This => self
                .lower_current_this(span)
                .expect("the decoder receiver is available"),
            DecoderValue::Singleton(ty) => self.decoding_singleton(*ty, span)?,
            DecoderValue::Container { companion, element } => {
                let receiver = self.decoding_singleton(*companion, span)?;
                let receiver = self.decoding_local(receiver, span, sink);
                let element = self.selected_decoder_value(context, element, span, sink)?;
                self.decoding_call(receiver, "decoder", vec![element], span, sink)?
            }
            DecoderValue::Tuple(elements) => {
                let decoder = self.core_coding_nominal("Decoder")?;
                let decoder = self
                    .apply_nominal_type(decoder, Vec::new())
                    .expect("Decoder is an ordinary interface");
                let function =
                    self.lower_synthesized_lambda(decoder, selected.ty, span, |state, decoder| {
                        let mut statements = Vec::new();
                        let value = state.decode_tuple(
                            context,
                            selected.ty,
                            elements,
                            decoder,
                            span,
                            &mut statements,
                        )?;
                        Some(crate::stmt::ValueBlock {
                            statements,
                            value: Some(value),
                        })
                    })?;
                let adapter = self.core_coding_nominal("DecodeFunction")?;
                let adapter = self
                    .apply_nominal_type(adapter, vec![selected.ty])
                    .expect("DecodeFunction has one unconstrained parameter");
                let record = self.decoding_primary(adapter, span)?;
                self.call_decoding_constructor(adapter, &record, vec![function], span, sink)?
            }
        };
        let interface = self
            .apply_nominal_type(context.decodable, vec![selected.ty])
            .expect("Decodable has one unconstrained parameter");
        Some(self.adapt_to(value, interface))
    }

    fn decoding_singleton(&mut self, ty: TypeId, span: Span) -> Option<hir::Expr> {
        let application = self
            .nominal_application(ty)
            .expect("a selected singleton has its complete application");
        if let Some(NominalTarget::Object(object)) = self.nominal_target_for_type(ty) {
            self.lower_singleton_application(object, application.arguments, span)
        } else {
            self.lower_imported_singleton_type(ty, span)
        }
    }

    pub(in crate::class::inheritance::decoding) fn decode_selected_value(
        &mut self,
        context: DecodeContext,
        selected: &SelectedDecoder,
        child: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        // The child is obtained before singleton initialization or codec construction.
        let child = self.decoding_local(child, span, sink);
        let codec = self.selected_decoder_value(context, selected, span, sink)?;
        let value = self.decoding_call(codec, "decode", vec![child], span, sink)?;
        Some(self.decoding_local(value, span, sink))
    }

    pub(in crate::class::inheritance::decoding) fn decode_tuple(
        &mut self,
        context: DecodeContext,
        ty: TypeId,
        codecs: &[SelectedDecoder],
        decoder: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let elements = self.decoding_call(decoder, "unkeyed", Vec::new(), span, sink)?;
        let elements = self.decoding_local(elements, span, sink);
        let mut values = Vec::with_capacity(codecs.len());
        for codec in codecs {
            let child = self.decoding_call(elements.clone(), "element", Vec::new(), span, sink)?;
            values.push(self.decode_selected_value(context, codec, child, span, sink)?);
        }
        self.end_decoding_container(elements, span, sink)?;
        Some(self.decoding_expr(hir::ExprKind::TupleLiteral(values), ty, span))
    }
}
