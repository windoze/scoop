use super::*;

impl Lowerer {
    pub(super) fn encode_enum(
        &mut self,
        enumeration: hir::EnumId,
        encodable: TypeId,
        span: Span,
    ) -> Option<ast::Block> {
        let variants = self.enums[enumeration].variants.clone();
        let mut arms = Vec::new();
        let mut names = std::collections::HashSet::new();
        for (index, variant) in variants.into_iter().enumerate() {
            let source = hir::EnumVariantRef::checked(&self.enums, enumeration, index as u32)
                .expect("the shape iterates this enum's variants");
            let variant_span = self.enum_variant_spans[&source];
            let name = self
                .encoding_wire_name(hir::SourceAnnotationTarget::Variant(source), &variant.name)
                .expect("Transient is not a variant annotation");
            if !names.insert(name.clone()) {
                self.error(
                    variant_span,
                    format!("automatic encode has duplicate variant name `{name}`"),
                );
            }
            let child = syntax::call(
                syntax::variable("__encoding_variants", variant_span),
                "field",
                vec![syntax::string(name, variant_span)],
                variant_span,
            );
            let mut fields = Vec::new();
            let mut patterns = Vec::new();
            let mut field_names = std::collections::HashSet::new();
            for (field_index, field) in variant.fields.iter().enumerate() {
                let source =
                    hir::EnumVariantFieldRef::checked(&self.enums, source, field_index as u32)
                        .expect("the shape iterates this variant's fields");
                let span = self.enum_variant_field_spans[&source];
                if let Some(name) = self.encoding_wire_name(
                    hir::SourceAnnotationTarget::VariantField(source),
                    &field.name,
                ) {
                    let binding = format!("__encoding_value_{field_index}");
                    self.check_encoding_field(&name, field.ty, encodable, span, &mut field_names);
                    fields.push((name, syntax::variable(&binding, span), span));
                    patterns.push(ast::Pattern::Binding(syntax::ident(&binding, span)));
                } else {
                    patterns.push(ast::Pattern::Wildcard { span });
                }
            }
            let body = if variant.style == hir::VariantStyle::Positional {
                syntax::sequence(
                    fields
                        .into_iter()
                        .map(|(_, value, span)| (value, span))
                        .collect(),
                    child,
                    variant_span,
                )
            } else {
                syntax::record(fields, child, variant_span)
            };
            let path = vec![syntax::ident(&variant.name, variant_span)];
            let pattern = if variant.style == hir::VariantStyle::Named {
                ast::Pattern::Named {
                    path,
                    fields: variant
                        .fields
                        .iter()
                        .zip(patterns)
                        .map(|(field, subpattern)| ast::FieldPattern {
                            field: syntax::ident(&field.name, variant_span),
                            subpattern: Box::new(subpattern),
                            span: variant_span,
                        })
                        .collect(),
                    rest: None,
                    span: variant_span,
                }
            } else {
                ast::Pattern::Positional {
                    path,
                    elements: patterns,
                    rest: None,
                    span: variant_span,
                }
            };
            arms.push(ast::WhenArm {
                pattern,
                guard: None,
                body,
                span: variant_span,
            });
        }
        Some(ast::Block {
            statements: vec![
                syntax::local(
                    "__encoding_variants",
                    syntax::call(syntax::variable("encoder", span), "keyed", vec![], span),
                    span,
                ),
                ast::Statement {
                    kind: ast::StatementKind::When(ast::When {
                        subject: ast::Expr::This { span },
                        arms,
                        else_body: None,
                        span,
                    }),
                    span,
                },
                syntax::statement(syntax::call(
                    syntax::variable("__encoding_variants", span),
                    "end",
                    vec![],
                    span,
                )),
            ],
            span,
        })
    }
}
