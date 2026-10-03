use super::*;

impl Lowerer {
    pub(super) fn bare_variant_pattern(
        &mut self,
        name: &ast::Ident,
        application: hir::EnumVariantApplication,
        ctx: PatternCtx,
    ) -> Option<hir::Pattern> {
        let variant = self.enum_variant(application);
        let count = variant.fields.len();
        if count != 0 {
            self.error(
                name.span,
                format!(
                    "variant `{}` of `{}` has {count} field(s); use `{}(...)` to match it",
                    name.text, variant.owner_name, name.text,
                ),
            );
            return None;
        }
        if !ctx.in_when {
            self.error(
                name.span,
                "refutable patterns are only allowed in `when`".into(),
            );
            return None;
        }
        Some(hir::Pattern::Variant {
            application,
            fields: Vec::new(),
        })
    }

    pub(super) fn lower_variant_positional_pattern(
        &mut self,
        application: hir::EnumVariantApplication,
        elements: &[ast::Pattern],
        rest: Option<Span>,
        span: Span,
        ctx: PatternCtx,
    ) -> Option<hir::Pattern> {
        if !ctx.in_when {
            self.error(span, "refutable patterns are only allowed in `when`".into());
            return None;
        }
        let variant = self.enum_variant(application);
        let description = variant_owner(&variant.owner_name, &variant.name);
        if variant.style == VariantStyle::Named {
            self.error(
                span,
                format!("{description} has named fields; use a named field pattern"),
            );
            return None;
        }
        let types = variant
            .fields
            .into_iter()
            .map(|(_, ty)| ty)
            .collect::<Vec<_>>();
        let fields =
            self.lower_positional_pattern(elements, rest, &types, &description, span, ctx)?;
        Some(hir::Pattern::Variant {
            application,
            fields,
        })
    }

    pub(super) fn lower_variant_named_pattern(
        &mut self,
        application: hir::EnumVariantApplication,
        fields: &[ast::FieldPattern],
        rest: Option<Span>,
        span: Span,
        ctx: PatternCtx,
    ) -> Option<hir::Pattern> {
        if !ctx.in_when {
            self.error(span, "refutable patterns are only allowed in `when`".into());
            return None;
        }
        let variant = self.enum_variant(application);
        let description = variant_owner(&variant.owner_name, &variant.name);
        if matches!(variant.style, VariantStyle::Unit | VariantStyle::Positional) {
            self.error(
                span,
                format!("{description} has no named fields; use a positional pattern"),
            );
            return None;
        }
        let fields =
            self.lower_named_fields(fields, rest, &variant.fields, &description, span, ctx)?;
        Some(hir::Pattern::Variant {
            application,
            fields,
        })
    }
}
