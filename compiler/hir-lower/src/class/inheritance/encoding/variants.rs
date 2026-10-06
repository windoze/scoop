use super::*;

impl Lowerer {
    pub(super) fn encode_enum(
        &mut self,
        context: CodingContext,
        value: hir::Expr,
        encoder: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<()> {
        let container = self.encoding_container(encoder, true, span, sink)?;
        let mut arms = Vec::new();
        let mut names = std::collections::HashSet::new();
        for variant in self.enum_variants(context.target) {
            let (wire, fields) = self.encoding_variant_names(&variant);
            self.check_encoding_wire_name(&wire, "variant", span, &mut names)?;
            let mut body = Vec::new();
            let name = self.coding_string(&wire, span);
            let child =
                self.coding_call(container.clone(), "field", vec![name], span, &mut body)?;
            let keyed = variant.style != hir::VariantStyle::Positional;
            let payload = self.encoding_container(child, keyed, span, &mut body)?;
            let mut patterns = Vec::new();
            let mut field_names = std::collections::HashSet::new();
            for (index, ((_, ty), wire)) in variant.fields.iter().zip(fields).enumerate() {
                let Some(wire) = wire else { continue };
                if keyed {
                    self.check_encoding_wire_name(&wire, "field", span, &mut field_names)?;
                }
                let local = self.alloc_hidden("encoded_payload", *ty);
                patterns.push((index as u32, hir::Pattern::Binding { local }));
                let field = self.coding_expr(hir::ExprKind::Local(local), *ty, span);
                self.encode_field(
                    context,
                    field,
                    payload.clone(),
                    keyed.then_some(wire.as_str()),
                    span,
                    &mut body,
                )?;
            }
            self.end_coding_container(payload, span, &mut body)?;
            arms.push(hir::WhenArm {
                pattern: hir::Pattern::Variant {
                    application: variant.application,
                    fields: patterns,
                },
                guard: None,
                body,
                span,
            });
        }
        sink.push(hir::Statement {
            kind: hir::StatementKind::When(hir::When {
                subject: value,
                arms,
                fallback: hir::WhenFallback::Impossible(
                    hir::ExhaustivenessProof::EnumPatternMatrix {
                        subject_ty: context.target,
                    },
                ),
            }),
            span,
        });
        self.end_coding_container(container, span, sink)
    }

    fn encoding_variant_names(
        &self,
        variant: &crate::types::EnumVariant,
    ) -> (String, Vec<Option<String>>) {
        let application = self
            .nominal_application(variant.application.owner)
            .expect("a variant belongs to its enum application");
        let index = self.enum_variant_index(variant.application);
        if let Some(enumeration) = self.source_enum_id(application.template) {
            let source = hir::EnumVariantRef::checked(&self.enums, enumeration, index)
                .expect("the variant belongs to its declaration");
            let wire = self
                .serialization_wire_name(
                    hir::SourceAnnotationTarget::Variant(source),
                    &variant.name,
                )
                .expect("variants cannot be Transient");
            let fields = variant
                .fields
                .iter()
                .enumerate()
                .map(|(index, (name, _))| {
                    let field =
                        hir::EnumVariantFieldRef::checked(&self.enums, source, index as u32)
                            .expect("the field belongs to its variant");
                    self.serialization_wire_name(
                        hir::SourceAnnotationTarget::VariantField(field),
                        name,
                    )
                })
                .collect();
            (wire, fields)
        } else {
            let wire = self
                .dependency_coding_wire_name(
                    hir::AnnotationTargetV1::Variant(variant.application.variant),
                    &variant.name,
                )
                .expect("variants cannot be Transient");
            let fields = variant
                .fields
                .iter()
                .enumerate()
                .map(|(field_index, (name, _))| {
                    let field = self.loaded_enum_definitions[&application.template]
                        .field_identity(index as usize, field_index);
                    self.dependency_coding_wire_name(
                        hir::AnnotationTargetV1::VariantField(field),
                        name,
                    )
                })
                .collect();
            (wire, fields)
        }
    }
}
