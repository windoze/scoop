//! Pattern target resolution and field-shape normalization.

use std::collections::HashSet;

use super::*;

impl Lowerer {
    pub(super) fn bare_variant_pattern(
        &mut self,
        name: &ast::Ident,
        application: hir::EnumApplicationId,
        variant: u32,
        ctx: PatternCtx,
    ) -> Option<hir::Pattern> {
        let enum_id = self.enum_applications[application].template;
        let field_count = self.enums[enum_id].variants[variant as usize].fields.len();
        if field_count != 0 {
            let enum_name = &self.enums[enum_id].name;
            let text = &name.text;
            self.error(
                name.span,
                format!(
                    "variant `{text}` of `{enum_name}` has {field_count} field(s); use `{text}(...)` to match it"
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
        Some(hir::Pattern::Variant {
            application,
            variant,
            fields: Vec::new(),
        })
    }

    pub(super) fn resolve_pattern_path(
        &mut self,
        path: &[ast::Ident],
        matched_ty: TypeId,
        span: Span,
    ) -> Option<PatternTarget> {
        match path {
            [] => match self.types[matched_ty].clone() {
                Type::Struct(application) => Some(PatternTarget::Struct(application)),
                _ => {
                    let found = self.type_name(matched_ty);
                    self.error(
                        span,
                        format!("a field pattern without a type name can only match a struct, found {found}"),
                    );
                    None
                }
            },
            [name] => match self.types[matched_ty].clone() {
                Type::Enum(application) => {
                    let enum_id = self.enum_applications[application].template;
                    let Some(variant) = self.find_variant(enum_id, &name.text) else {
                        let enum_name = self.enums[enum_id].name.clone();
                        self.error(
                            name.span,
                            format!("enum `{enum_name}` has no variant `{}`", name.text),
                        );
                        return None;
                    };
                    Some(PatternTarget::Variant(application, variant))
                }
                Type::Struct(application)
                    if self.structs[self.struct_applications[application].template].name
                        == name.text =>
                {
                    Some(PatternTarget::Struct(application))
                }
                _ => {
                    let found = self.type_name(matched_ty);
                    self.error(
                        name.span,
                        format!(
                            "pattern `{}` does not match a subject of type {found}",
                            name.text
                        ),
                    );
                    None
                }
            },
            [enum_name, variant_name] => {
                let Some(&enum_id) = self.enums_by_name.get(&enum_name.text) else {
                    self.error(enum_name.span, format!("unknown enum `{}`", enum_name.text));
                    return None;
                };
                let Some(variant) = self.find_variant(enum_id, &variant_name.text) else {
                    self.error(
                        variant_name.span,
                        format!(
                            "enum `{}` has no variant `{}`",
                            enum_name.text, variant_name.text
                        ),
                    );
                    return None;
                };
                match self.types[matched_ty].clone() {
                    Type::Enum(application)
                        if self.enum_applications[application].template == enum_id =>
                    {
                        Some(PatternTarget::Variant(application, variant))
                    }
                    _ => {
                        let found = self.type_name(matched_ty);
                        self.error(
                            span,
                            format!(
                                "pattern `{}.{}` does not match a subject of type {found}",
                                enum_name.text, variant_name.text
                            ),
                        );
                        None
                    }
                }
            }
            _ => {
                self.error(span, "invalid pattern path".to_string());
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
        let mut fields = Vec::with_capacity(count);
        for (position, element) in elements.iter().enumerate() {
            let field_index = if position < leading {
                position
            } else {
                total - (count - position)
            };
            let sub = self.lower_pattern(element, field_types[field_index], ctx)?;
            fields.push((field_index as u32, sub));
        }
        Some(fields)
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
        if rest.is_none() && fields.len() < field_types.len() {
            self.error(
                span,
                format!("pattern does not list all fields of {owner}; add `..` to ignore the rest"),
            );
            return None;
        }
        let mut seen = HashSet::new();
        let mut pairs = Vec::with_capacity(fields.len());
        for field in fields {
            if field.name.text == "_" {
                self.error(
                    field.name.span,
                    "`_` is not allowed in a field pattern".to_string(),
                );
                return None;
            }
            if !seen.insert(field.name.text.clone()) {
                self.error(
                    field.name.span,
                    format!("duplicate field `{}` in pattern", field.name.text),
                );
                return None;
            }
            let Some(index) = field_types
                .iter()
                .position(|(name, _)| name == &field.name.text)
            else {
                self.error(
                    field.name.span,
                    format!("{owner} has no field `{}`", field.name.text),
                );
                return None;
            };
            let binding = field.rename.as_ref().unwrap_or(&field.name);
            let local = self.bind_local(binding, field_types[index].1, ctx.mutable)?;
            pairs.push((index as u32, hir::Pattern::Binding { local }));
        }
        pairs.sort_by_key(|(index, _)| *index);
        Some(pairs)
    }

    pub(super) fn variant_field_types(
        &mut self,
        enum_id: hir::EnumId,
        variant: u32,
        type_args: &[TypeId],
    ) -> Vec<TypeId> {
        let field_types: Vec<TypeId> = self.enums[enum_id].variants[variant as usize]
            .fields
            .iter()
            .map(|field| field.ty)
            .collect();
        field_types
            .into_iter()
            .map(|ty| self.instantiate_ty(ty, type_args))
            .collect()
    }

    pub(super) fn variant_named_field_types(
        &mut self,
        enum_id: hir::EnumId,
        variant: u32,
        type_args: &[TypeId],
    ) -> Vec<(String, TypeId)> {
        let fields: Vec<(String, TypeId)> = self.enums[enum_id].variants[variant as usize]
            .fields
            .iter()
            .map(|field| (field.name.clone(), field.ty))
            .collect();
        fields
            .into_iter()
            .map(|(name, ty)| (name, self.instantiate_ty(ty, type_args)))
            .collect()
    }
}

fn pattern_span(pattern: &ast::Pattern) -> Span {
    match pattern {
        ast::Pattern::Binding(name) => name.span,
        ast::Pattern::Wildcard { span }
        | ast::Pattern::Literal { span, .. }
        | ast::Pattern::Positional { span, .. }
        | ast::Pattern::Named { span, .. }
        | ast::Pattern::Tuple { span, .. } => *span,
    }
}
