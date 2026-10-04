//! Pattern target resolution and field-shape normalization.

use std::collections::HashSet;

use super::*;

mod targets;

impl Lowerer {
    pub(super) fn lower_struct_positional_pattern(
        &mut self,
        owner: TypeId,
        elements: &[ast::Pattern],
        rest: Option<Span>,
        span: Span,
        ctx: PatternCtx,
    ) -> Option<hir::Pattern> {
        let structure = self
            .struct_fields(owner)
            .expect("a struct pattern has complete fields");
        let description = format!("struct `{}`", structure.name);
        let types = structure
            .fields
            .iter()
            .map(|field| field.ty)
            .collect::<Vec<_>>();
        let fields =
            self.lower_positional_pattern(elements, rest, &types, &description, span, ctx)?;
        Some(hir::Pattern::Struct { owner, fields })
    }

    pub(super) fn lower_struct_named_pattern(
        &mut self,
        owner: TypeId,
        fields: &[ast::FieldPattern],
        rest: Option<Span>,
        span: Span,
        ctx: PatternCtx,
    ) -> Option<hir::Pattern> {
        let structure = self
            .struct_fields(owner)
            .expect("a struct pattern has complete fields");
        let description = format!("struct `{}`", structure.name);
        let types = structure
            .fields
            .into_iter()
            .map(|field| (field.name, field.ty))
            .collect::<Vec<_>>();
        let fields = self.lower_named_fields(fields, rest, &types, &description, span, ctx)?;
        Some(hir::Pattern::Struct { owner, fields })
    }

    pub(super) fn resolve_pattern_path(
        &mut self,
        path: &[ast::Ident],
        matched_ty: TypeId,
        span: Span,
    ) -> Option<PatternTarget> {
        if self.is_char_type(matched_ty) {
            self.error(span, "intrinsic type `Char` cannot be destructured".into());
            return None;
        }
        match self.types[matched_ty] {
            Type::Struct(_) => {
                if path.is_empty() || self.pattern_type_name_matches(path, matched_ty)? {
                    Some(PatternTarget::Struct(matched_ty))
                } else {
                    self.pattern_type_mismatch(path, matched_ty, span);
                    None
                }
            }
            Type::Enum(_) => self.resolve_enum_pattern_path(path, matched_ty, span),
            _ if path.is_empty() => {
                self.error(
                    span,
                    format!(
                        "a field pattern without a type name can only match a struct, found {}",
                        self.type_name(matched_ty),
                    ),
                );
                None
            }
            Type::Ptr(_) | Type::FunPtr(_) => {
                self.error(
                    span,
                    format!(
                        "intrinsic pointer type `{}` cannot be destructured",
                        self.type_name(matched_ty),
                    ),
                );
                None
            }
            _ => {
                self.pattern_type_mismatch(path, matched_ty, span);
                None
            }
        }
    }

    pub(super) fn lower_positional_pattern(
        &mut self,
        elements: &[ast::Pattern],
        rest: Option<Span>,
        field_types: &[TypeId],
        owner: &str,
        span: Span,
        ctx: PatternCtx,
    ) -> Option<Vec<(u32, hir::Pattern)>> {
        let total = field_types.len();
        let indices = self.positional_pattern_indices(elements, rest, total, owner, span)?;
        let mut fields = Vec::with_capacity(elements.len());
        for (element, field_index) in elements.iter().zip(indices) {
            let sub = self.lower_pattern_inner(element, field_types[field_index], ctx)?;
            fields.push((field_index as u32, sub));
        }
        Some(fields)
    }

    pub(crate) fn positional_pattern_indices(
        &mut self,
        elements: &[ast::Pattern],
        rest: Option<Span>,
        total: usize,
        owner: &str,
        span: Span,
    ) -> Option<Vec<usize>> {
        let count = elements.len();
        if (rest.is_none() && count != total) || (rest.is_some() && count > total) {
            self.error(
                span,
                format!("pattern has {count} element(s), but {owner} has {total}"),
            );
            return None;
        }
        let leading = match rest {
            Some(rest_span) => elements
                .iter()
                .take_while(|element| pattern_span(element).start < rest_span.start)
                .count(),
            None => count,
        };
        Some(
            (0..count)
                .map(|position| {
                    if position < leading {
                        position
                    } else {
                        total - (count - position)
                    }
                })
                .collect(),
        )
    }

    pub(super) fn lower_named_fields(
        &mut self,
        fields: &[ast::FieldPattern],
        rest: Option<Span>,
        field_types: &[(String, TypeId)],
        owner: &str,
        span: Span,
        ctx: PatternCtx,
    ) -> Option<Vec<(u32, hir::Pattern)>> {
        let names = field_types
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>();
        let indices = self.named_pattern_indices(fields, rest, &names, owner, span)?;
        let mut normalized = (0..field_types.len())
            .map(|index| (index as u32, hir::Pattern::Wildcard))
            .collect::<Vec<_>>();
        for (field, index) in fields.iter().zip(indices) {
            normalized[index].1 =
                self.lower_pattern_inner(&field.subpattern, field_types[index].1, ctx)?;
        }
        Some(normalized)
    }

    pub(super) fn named_pattern_indices(
        &mut self,
        fields: &[ast::FieldPattern],
        rest: Option<Span>,
        names: &[&str],
        owner: &str,
        span: Span,
    ) -> Option<Vec<usize>> {
        if rest.is_none() && fields.len() < names.len() {
            self.error(
                span,
                format!("pattern does not list all fields of {owner}; add `..` to ignore the rest"),
            );
            return None;
        }
        let mut seen = HashSet::new();
        let mut indices = Vec::with_capacity(fields.len());
        for field in fields {
            if field.field.text == "_" {
                self.error(
                    field.field.span,
                    "`_` is not allowed in a field pattern".into(),
                );
                return None;
            }
            if !seen.insert(field.field.text.as_str()) {
                self.error(
                    field.field.span,
                    format!("duplicate field `{}` in pattern", field.field.text),
                );
                return None;
            }
            let Some(index) = names.iter().position(|name| *name == field.field.text) else {
                self.error(
                    field.field.span,
                    format!("{owner} has no field `{}`", field.field.text),
                );
                return None;
            };
            indices.push(index);
        }
        Some(indices)
    }
}

pub(super) fn pattern_span(pattern: &ast::Pattern) -> Span {
    match pattern {
        ast::Pattern::Binding(name) => name.span,
        ast::Pattern::Wildcard { span }
        | ast::Pattern::Literal { span, .. }
        | ast::Pattern::Positional { span, .. }
        | ast::Pattern::Named { span, .. }
        | ast::Pattern::Tuple { span, .. } => *span,
    }
}
