use super::*;

impl Lowerer {
    pub(super) fn encode_enum(
        &mut self,
        application: hir::EnumApplicationId,
        context: CodingContext,
        value: hir::Expr,
        encoder: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<()> {
        let container = self.encoding_container(encoder, true, span, sink)?;
        let mut arms = Vec::new();
        let mut names = std::collections::HashSet::new();
        for variant in self.enum_variants(application) {
            let (wire, fields) = self.encoding_variant_names(&variant, span)?;
            self.check_encoding_wire_name(&wire, "variant", span, &mut names)?;
            let mut body = Vec::new();
            let name = self.coding_string(&wire, span);
            let child =
                self.coding_call(container.clone(), "field", vec![name], span, &mut body)?;
            let keyed = variant.style != hir::VariantStyle::Positional;
            let payload = self.encoding_container(child, keyed, span, &mut body)?;
            let mut patterns = Vec::new();
            let mut field_names = std::collections::HashSet::new();
            for (index, ((_, ty), member)) in variant.fields.iter().zip(fields).enumerate() {
                let Some(member) = member else { continue };
                if keyed {
                    self.check_encoding_wire_name(&member.wire, "field", span, &mut field_names)?;
                }
                let local = self.alloc_hidden("encoded_payload", *ty);
                patterns.push((index as u32, hir::Pattern::Binding { local }));
                let field = self.coding_expr(hir::ExprKind::Local(local), *ty, span);
                self.encode_field(
                    context,
                    field,
                    payload.clone(),
                    keyed.then_some(&member),
                    span,
                    &mut body,
                )?;
            }
            self.end_coding_container(payload, span, &mut body)?;
            arms.push(hir::WhenArm {
                condition: hir::WhenCondition::Case(hir::Pattern::Variant {
                    application: variant.application,
                    fields: patterns,
                }),
                guard: None,
                body,
                span,
            });
        }
        sink.push(hir::Statement {
            kind: hir::StatementKind::When(hir::When::Subject {
                subject: value,
                arms,
                fallback: hir::WhenFallback::Impossible(
                    hir::ExhaustivenessProof::EnumPatternMatrix,
                ),
            }),
            span,
        });
        self.end_coding_container(container, span, sink)
    }

    fn encoding_variant_names(
        &mut self,
        variant: &crate::types::EnumVariant,
        span: Span,
    ) -> Option<(String, Vec<Option<EncodingMember>>)> {
        let application = self.enum_applications[variant.application.owner].clone();
        let index = self.enum_variant_index(variant.application);
        if let Some(enumeration) = self.source_enum_id(application.template) {
            let source = hir::EnumVariantRef::checked(&self.enums, enumeration, index)
                .expect("the variant belongs to its declaration");
            let wire = self
                .serialization_wire_name(
                    hir::SourceAnnotationTarget::Variant(source),
                    &variant.name,
                )
                .expect("variants cannot use Serialize(ignore = true)");
            let omissions = self.current_coding_omissions(
                crate::call_resolution::candidates::NominalConstructorSource::Variant(source),
                span,
            )?;
            let fields = variant
                .fields
                .iter()
                .enumerate()
                .zip(omissions)
                .map(|((index, (name, _)), omit_none)| {
                    let field =
                        hir::EnumVariantFieldRef::checked(&self.enums, source, index as u32)
                            .expect("the field belongs to its variant");
                    let config =
                        self.serialization_config(hir::SourceAnnotationTarget::VariantField(field));
                    config.wire_name(name).map(|wire| EncodingMember {
                        wire,
                        omit_none: omit_none && !config.required && !config.emit_null,
                    })
                })
                .collect();
            Some((wire, fields))
        } else {
            let wire = self
                .dependency_coding_wire_name(
                    hir::AnnotationTargetV1::Variant(variant.application.variant),
                    &variant.name,
                )
                .expect("variants cannot use Serialize(ignore = true)");
            let omissions = self.dependency_coding_omissions(
                scoop_identity::CallableTemplateOrigin::VariantConstructor(
                    variant.application.variant,
                ),
            );
            let fields = variant
                .fields
                .iter()
                .enumerate()
                .zip(omissions)
                .map(|((field_index, (name, _)), omit_none)| {
                    let field = self.loaded_enum_definitions[&application.template]
                        .field_identity(index as usize, field_index);
                    let config = self.dependency_serialization_config(
                        hir::AnnotationTargetV1::VariantField(field),
                    );
                    config.wire_name(name).map(|wire| EncodingMember {
                        wire,
                        omit_none: omit_none && !config.required && !config.emit_null,
                    })
                })
                .collect();
            Some((wire, fields))
        }
    }
}
