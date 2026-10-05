use super::*;

impl Lowerer {
    pub(super) fn decode_variants(
        &mut self,
        context: DecodeContext,
        decoder: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let decoder = self.decoding_local(decoder, span, sink);
        let variants = self.decoding_call(decoder.clone(), "keyed", Vec::new(), span, sink)?;
        let variants = self.decoding_local(variants, span, sink);
        let keys = self.decoding_property(variants.clone(), "keys", span)?;
        let keys = self.decoding_local(keys, span, sink);
        let count = self.decoding_property(keys.clone(), "size", span)?;
        let one = self.decoding_long(1, span);
        let correct = self.decoding_call(count, "equals", vec![one], span, sink)?;
        let invalid = self.decoding_expr(
            hir::ExprKind::Unary {
                op: hir::UnOp::Not,
                operand: Box::new(correct),
            },
            self.boolean,
            span,
        );
        let failure =
            self.decoding_failure(decoder.clone(), "expected exactly one enum variant", span)?;
        sink.push(hir::Statement {
            kind: hir::StatementKind::If {
                cond: invalid,
                then_body: failure,
                else_body: None,
            },
            span,
        });
        let index = self.decoding_long(0, span);
        let name = self.decoding_call(keys, "get", vec![index], span, sink)?;
        let name = self.decoding_local(name, span, sink);
        let result = self.alloc_hidden("decoded_variant", context.result);
        let mut names = std::collections::HashSet::new();
        let mut branches = Vec::new();
        for variant in self.enum_variants(context.result) {
            let (wire, record) = self.decoding_variant_shape(&variant, span)?;
            if !names.insert(wire.clone()) {
                self.error(
                    span,
                    format!("automatic decode has duplicate variant name `{wire}`"),
                );
                return None;
            }
            let wire = self.decoding_string(&wire, span);
            let mut setup = Vec::new();
            let condition =
                self.decoding_call(name.clone(), "equals", vec![wire], span, &mut setup)?;
            let mut body = Vec::new();
            let child = self.decoding_call(
                variants.clone(),
                "required",
                vec![name.clone()],
                span,
                &mut body,
            )?;
            let inputs = self.read_decoding_record(context, &record, child, span, &mut body)?;
            self.end_decoding_container(variants.clone(), span, &mut body)?;
            let value = if variant.style == hir::VariantStyle::Unit {
                self.decoding_expr(
                    hir::ExprKind::VariantConstruct {
                        variant: variant.application,
                        args: Vec::new(),
                    },
                    context.result,
                    span,
                )
            } else {
                self.finish_decoding_record(context.result, &record, inputs, span, &mut body)?
            };
            body.push(hir::Statement {
                kind: hir::StatementKind::ValDecl {
                    pattern: hir::Pattern::Binding { local: result },
                    init: value,
                },
                span,
            });
            branches.push((setup, condition, body));
        }
        let mut tail = self.decoding_failure(decoder, "unknown enum variant", span)?;
        for (mut setup, condition, body) in branches.into_iter().rev() {
            setup.push(hir::Statement {
                kind: hir::StatementKind::If {
                    cond: condition,
                    then_body: body,
                    else_body: Some(tail),
                },
                span,
            });
            tail = setup;
        }
        sink.extend(tail);
        Some(self.decoding_expr(hir::ExprKind::Local(result), context.result, span))
    }

    fn decoding_variant_shape(
        &mut self,
        variant: &crate::types::EnumVariant,
        span: Span,
    ) -> Option<(String, DecodeRecord)> {
        let application = self
            .nominal_application(variant.application.owner)
            .expect("a variant belongs to a nominal application");
        let index = self.enum_variant_index(variant.application);
        let keyed = variant.style != hir::VariantStyle::Positional;
        if let Some(enumeration) = self.source_enum_id(application.template) {
            let source = hir::EnumVariantRef::checked(&self.enums, enumeration, index)
                .expect("the variant belongs to its declaration");
            let wire = self
                .serialization_wire_name(
                    hir::SourceAnnotationTarget::Variant(source),
                    &variant.name,
                )
                .expect("variants cannot be Transient");
            let mut record = self.current_decoding_record(
                variant.application.owner,
                NominalConstructorSource::Variant(source),
                keyed,
                span,
            );
            for (index, parameter) in record.parameters.iter_mut().enumerate() {
                let field = hir::EnumVariantFieldRef::checked(&self.enums, source, index as u32)
                    .expect("a variant parameter maps to its field");
                parameter.wire = self.serialization_wire_name(
                    hir::SourceAnnotationTarget::VariantField(field),
                    &parameter.name,
                );
            }
            Some((wire, record))
        } else {
            let wire = self
                .dependency_decoding_wire_name(
                    hir::AnnotationTargetV1::Variant(variant.application.variant),
                    &variant.name,
                )
                .expect("variants cannot be Transient");
            let mut record = self.dependency_decoding_record(
                variant.application.owner,
                scoop_identity::CallableTemplateOrigin::VariantConstructor(
                    variant.application.variant,
                ),
                keyed,
                span,
            )?;
            for (field_index, parameter) in record.parameters.iter_mut().enumerate() {
                let field = self.loaded_enum_definitions[&application.template]
                    .field_identity(index as usize, field_index);
                parameter.wire = self.dependency_decoding_wire_name(
                    hir::AnnotationTargetV1::VariantField(field),
                    &parameter.name,
                );
            }
            Some((wire, record))
        }
    }
}
