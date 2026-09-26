//! Dependency enum patterns use the same field normalization and coverage rules.

use super::*;

impl Lowerer {
    pub(super) fn resolve_imported_enum_pattern_path(
        &mut self,
        path: &[ast::Ident],
        owner: TypeId,
        span: Span,
    ) -> Option<PatternTarget> {
        let Some((name, qualifier)) = path.split_last() else {
            self.error(
                span,
                format!(
                    "a field pattern without a type name can only match a struct, found {}",
                    self.type_name(owner)
                ),
            );
            return None;
        };
        if !qualifier.is_empty() {
            let kind = match qualifier {
                [name] => ast::TypeRefKind::Named(name.clone()),
                _ => ast::TypeRefKind::Qualified {
                    path: qualifier.to_vec(),
                    arguments: Vec::new(),
                },
            };
            let reference = ast::TypeRef {
                kind,
                span: Span::new(qualifier[0].span.start, qualifier.last().unwrap().span.end),
            };
            let resolved = self.resolve_type_ref(&reference)?;
            if !self.types_equal(resolved, owner) {
                self.error(
                    span,
                    format!(
                        "pattern `{}` does not match a subject of type {}",
                        path.iter()
                            .map(|name| name.text.as_str())
                            .collect::<Vec<_>>()
                            .join("."),
                        self.type_name(owner)
                    ),
                );
                return None;
            }
        }
        let Type::ImportedEnum(enumeration) = &self.types[owner] else {
            unreachable!("dependency pattern resolution retains its enum subject")
        };
        let Some(variant) = enumeration
            .variants
            .iter()
            .position(|variant| variant.name == name.text)
        else {
            self.error(
                name.span,
                format!(
                    "enum `{}` has no variant `{}`",
                    enumeration.declaration.name(),
                    name.text
                ),
            );
            return None;
        };
        Some(PatternTarget::ImportedVariant { owner, variant })
    }

    pub(super) fn bare_imported_variant_pattern(
        &mut self,
        name: &ast::Ident,
        owner: TypeId,
        index: usize,
        ctx: PatternCtx,
    ) -> Option<hir::Pattern> {
        let Type::ImportedEnum(enumeration) = &self.types[owner] else {
            unreachable!("a dependency variant belongs to an enum subject")
        };
        let variant = &enumeration.variants[index];
        let field_count = variant.fields.len();
        if field_count != 0 {
            self.error(
                name.span,
                format!(
                    "variant `{}` of `{}` has {field_count} field(s); use `{}(...)` to match it",
                    name.text,
                    enumeration.declaration.name(),
                    name.text
                ),
            );
            return None;
        }
        if !ctx.in_when {
            self.error(
                name.span,
                "refutable patterns are only allowed in `when`".to_string(),
            );
            return None;
        }
        Some(hir::Pattern::ImportedVariant {
            owner,
            variant: variant.identity,
            fields: Vec::new(),
        })
    }

    pub(super) fn imported_positional_variant_pattern(
        &mut self,
        owner: TypeId,
        index: usize,
        elements: &[ast::Pattern],
        rest: Option<Span>,
        span: Span,
        ctx: PatternCtx,
    ) -> Option<hir::Pattern> {
        if !ctx.in_when {
            self.error(
                span,
                "refutable patterns are only allowed in `when`".to_string(),
            );
            return None;
        }
        let Type::ImportedEnum(enumeration) = self.types[owner].clone() else {
            unreachable!("a dependency variant belongs to an enum subject")
        };
        let variant = &enumeration.variants[index];
        let description = variant_owner(enumeration.declaration.name(), &variant.name);
        if variant.style == hir::EnumSourceVariantStyleV1::Named {
            self.error(
                span,
                format!("{description} has named fields; use a named field pattern"),
            );
            return None;
        }
        let field_types = variant
            .fields
            .iter()
            .map(|field| field.ty)
            .collect::<Vec<_>>();
        let fields =
            self.lower_positional_pattern(elements, rest, &field_types, &description, span, ctx)?;
        Some(hir::Pattern::ImportedVariant {
            owner,
            variant: variant.identity,
            fields,
        })
    }

    pub(super) fn imported_named_variant_pattern(
        &mut self,
        owner: TypeId,
        index: usize,
        fields: &[ast::FieldPattern],
        rest: Option<Span>,
        span: Span,
        ctx: PatternCtx,
    ) -> Option<hir::Pattern> {
        if !ctx.in_when {
            self.error(
                span,
                "refutable patterns are only allowed in `when`".to_string(),
            );
            return None;
        }
        let Type::ImportedEnum(enumeration) = self.types[owner].clone() else {
            unreachable!("a dependency variant belongs to an enum subject")
        };
        let variant = &enumeration.variants[index];
        let description = variant_owner(enumeration.declaration.name(), &variant.name);
        if matches!(
            variant.style,
            hir::EnumSourceVariantStyleV1::Unit | hir::EnumSourceVariantStyleV1::Positional
        ) {
            self.error(
                span,
                format!("{description} has no named fields; use a positional pattern"),
            );
            return None;
        }
        let named_fields = variant
            .fields
            .iter()
            .map(|field| (field.name.clone(), field.ty))
            .collect::<Vec<_>>();
        let fields =
            self.lower_named_fields(fields, rest, &named_fields, &description, span, ctx)?;
        Some(hir::Pattern::ImportedVariant {
            owner,
            variant: variant.identity,
            fields,
        })
    }
}
