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
                Type::Struct(application) => {
                    let struct_id = self.struct_applications[application].template;
                    if let Some(target) = self.lexical_nested_nominal_target(&name.text) {
                        if target == crate::NominalTarget::Struct(struct_id) {
                            return Some(PatternTarget::Struct(application));
                        }
                        let found = self.type_name(matched_ty);
                        self.error(
                            name.span,
                            format!(
                                "pattern `{}` does not match a subject of type {found}",
                                name.text
                            ),
                        );
                        return None;
                    }
                    if self.structs[struct_id].name == name.text {
                        return Some(PatternTarget::Struct(application));
                    }
                    if self.source_type_alias_named(&name.text).is_some() {
                        let target = self.resolve_type_alias_reference(name, false)?;
                        if self.types_equal(target, matched_ty) {
                            let Type::Struct(application) = self.types[target] else {
                                unreachable!("a type equal to a struct has struct representation")
                            };
                            return Some(PatternTarget::Struct(application));
                        }
                    }
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
                Type::Ptr(_) | Type::FunPtr(_) => {
                    let found = self.type_name(matched_ty);
                    self.error(
                        span,
                        format!("intrinsic pointer type `{found}` cannot be destructured"),
                    );
                    None
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
                let lexical_target = self.lexical_nested_nominal_target(&enum_name.text);
                let (enum_id, required_type) = if let Some(target) = lexical_target {
                    let crate::NominalTarget::Enum(enum_id) = target else {
                        self.error(
                            enum_name.span,
                            format!("type `{}` does not name an enum", enum_name.text),
                        );
                        return None;
                    };
                    (enum_id, None)
                } else if self.source_type_alias_named(&enum_name.text).is_some() {
                    let target = self.resolve_type_alias_reference(enum_name, false)?;
                    let Type::Enum(application) = self.types[target] else {
                        self.error(
                            enum_name.span,
                            format!("typealias `{}` does not name an enum", enum_name.text),
                        );
                        return None;
                    };
                    (self.enum_applications[application].template, Some(target))
                } else {
                    let enum_id = self
                        .first_nominal_layer(&enum_name.text)
                        .and_then(|(_, candidates)| candidates.first().copied())
                        .and_then(|target| match target {
                            crate::NominalTarget::Enum(id) => Some(id),
                            _ => None,
                        });
                    let Some(enum_id) = enum_id else {
                        self.error(enum_name.span, format!("unknown enum `{}`", enum_name.text));
                        return None;
                    };
                    (enum_id, None)
                };
                if required_type.is_some_and(|required| !self.types_equal(required, matched_ty)) {
                    let found = self.type_name(matched_ty);
                    self.error(
                        span,
                        format!(
                            "pattern `{}.{}` does not match a subject of type {found}",
                            enum_name.text, variant_name.text
                        ),
                    );
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
        if rest.is_none() && fields.len() < field_types.len() {
            self.error(
                span,
                format!("pattern does not list all fields of {owner}; add `..` to ignore the rest"),
            );
            return None;
        }
        let mut seen = HashSet::new();
        let mut normalized: Vec<(u32, hir::Pattern)> = (0..field_types.len())
            .map(|index| (index as u32, hir::Pattern::Wildcard))
            .collect();
        for field in fields {
            if field.field.text == "_" {
                self.error(
                    field.field.span,
                    "`_` is not allowed in a field pattern".to_string(),
                );
                return None;
            }
            if !seen.insert(field.field.text.clone()) {
                self.error(
                    field.field.span,
                    format!("duplicate field `{}` in pattern", field.field.text),
                );
                return None;
            }
            let Some(index) = field_types
                .iter()
                .position(|(name, _)| name == &field.field.text)
            else {
                self.error(
                    field.field.span,
                    format!("{owner} has no field `{}`", field.field.text),
                );
                return None;
            };
            normalized[index].1 =
                self.lower_pattern_inner(&field.subpattern, field_types[index].1, ctx)?;
        }
        Some(normalized)
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
