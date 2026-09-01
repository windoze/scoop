//! Pattern lowering for `when` arms and destructuring declarations
//! (spec 4.6 / 5, milestone4 DESIGN.md 3.2).
//!
//! Surface patterns (`ast::Pattern`) leave the variant/struct split
//! unresolved; here every pattern is resolved against the type of the
//! value it matches, producing `hir::Pattern` with declaration-order
//! field indices and `LocalId` bindings. Named and positional forms
//! are both normalized to `(field index, subpattern)` pairs; tuple
//! patterns are padded with wildcards for the positions a `..` skips.
//!
//! Two contexts share the machinery:
//!
//! - `when` arms (`in_when: true`): refutable patterns (enum variants,
//!   literals) are allowed, bindings are immutable, and exhaustiveness
//!   is checked over the whole arm list (`check_exhaustiveness`);
//! - `val` / `var` declarations (`in_when: false`): only irrefutable
//!   patterns (bindings, wildcards, tuple and struct patterns) are
//!   allowed — an enum variant or literal pattern is diagnosed with
//!   "refutable patterns are only allowed in `when`".
//!
//! The `..` rest marker's *position* is not explicit in the AST
//! (`rest` is just its span); the split between leading and trailing
//! elements is recovered by comparing element spans against it (the
//! parser assigns spans in source order).

use std::collections::HashSet;

use scoop_ast as ast;
use scoop_hir as hir;

use ast::Span;
use hir::{Type, TypeId};

use crate::{Lowerer, VariantStyle};

/// Where a pattern appears (spec 4.6 / 5).
#[derive(Clone, Copy)]
pub(crate) struct PatternCtx {
    /// `var` declarations bind mutably; everything else immutably.
    pub(crate) mutable: bool,
    /// `when` arms allow refutable patterns (enum variants, literals);
    /// `val` / `var` declarations do not.
    pub(crate) in_when: bool,
}

impl Lowerer {
    /// Lower a pattern against the type of the value it matches.
    /// Bindings are declared in the current scope. Records a
    /// diagnostic and returns `None` on error.
    pub(crate) fn lower_pattern(
        &mut self,
        pattern: &ast::Pattern,
        matched_ty: TypeId,
        ctx: PatternCtx,
    ) -> Option<hir::Pattern> {
        match pattern {
            ast::Pattern::Binding(name) => {
                // A bare identifier that names a unit variant of the
                // matched enum is a variant pattern (spec 5.1);
                // anything else binds (spec 5 "binding priority").
                if let Type::Enum(application) = self.types[matched_ty] {
                    let enum_id = self.enum_applications[application].template;
                    if let Some(variant) = self.find_variant(enum_id, &name.text) {
                        return self.bare_variant_pattern(name, application, variant, ctx);
                    }
                }
                let local = self.bind_local(name, matched_ty, ctx.mutable)?;
                Some(hir::Pattern::Binding { local })
            }
            ast::Pattern::Wildcard { .. } => Some(hir::Pattern::Wildcard),
            ast::Pattern::Literal { expr, span } => {
                if !ctx.in_when {
                    self.error(
                        *span,
                        "refutable patterns are only allowed in `when`".to_string(),
                    );
                    return None;
                }
                if !is_literal_expr(expr) {
                    // The parser only puts literals here; enforce it so
                    // `hir::Pattern::Literal`'s invariant holds.
                    self.error(*span, "expected a literal pattern".to_string());
                    return None;
                }
                let mut sink = Vec::new();
                let literal = self.lower_expr(expr, &mut sink, Some(matched_ty))?;
                if !self.types_equal(literal.ty, matched_ty) {
                    let expected = self.type_name(matched_ty);
                    let found = self.type_name(literal.ty);
                    self.error(
                        *span,
                        format!("literal pattern of type {found} cannot match {expected}"),
                    );
                    return None;
                }
                Some(hir::Pattern::Literal(literal))
            }
            ast::Pattern::Tuple {
                elements,
                rest,
                span,
            } => match self.types[matched_ty].clone() {
                Type::Tuple(element_types) => {
                    let owner = format!("tuple of type {}", self.type_name(matched_ty));
                    let pairs = self.lower_positional_pattern(
                        elements,
                        *rest,
                        &element_types,
                        &owner,
                        *span,
                        ctx,
                    )?;
                    // Normalize to full length: `..`-skipped positions
                    // become wildcards.
                    let mut full: Vec<hir::Pattern> = (0..element_types.len())
                        .map(|_| hir::Pattern::Wildcard)
                        .collect();
                    for (index, sub) in pairs {
                        full[index as usize] = sub;
                    }
                    Some(hir::Pattern::Tuple(full))
                }
                // A positional struct pattern without the type prefix
                // (spec 5.3: `(x, ..)` against a struct subject).
                Type::Struct(application) => {
                    let application_value = self.struct_applications[application].clone();
                    let struct_id = application_value.template;
                    let owner = format!("struct `{}`", self.structs[struct_id].name);
                    let declared: Vec<TypeId> = self.structs[struct_id]
                        .fields
                        .iter()
                        .map(|f| f.ty)
                        .collect();
                    let field_types: Vec<TypeId> = declared
                        .into_iter()
                        .map(|ty| self.instantiate_ty(ty, &application_value.arguments))
                        .collect();
                    let fields = self.lower_positional_pattern(
                        elements,
                        *rest,
                        &field_types,
                        &owner,
                        *span,
                        ctx,
                    )?;
                    Some(hir::Pattern::Struct {
                        application,
                        fields,
                    })
                }
                _ => {
                    let found = self.type_name(matched_ty);
                    self.error(
                        *span,
                        format!("tuple pattern does not match a subject of type {found}"),
                    );
                    None
                }
            },
            ast::Pattern::Positional {
                path,
                elements,
                rest,
                span,
            } => {
                let target = self.resolve_pattern_path(path, matched_ty, *span)?;
                match target {
                    PatternTarget::Variant(application, variant) => {
                        let application_value = self.enum_applications[application].clone();
                        let enum_id = application_value.template;
                        if !ctx.in_when {
                            self.error(
                                *span,
                                "refutable patterns are only allowed in `when`".to_string(),
                            );
                            return None;
                        }
                        let style = self.variant_styles[&(enum_id, variant)];
                        if style == VariantStyle::Named {
                            let owner = variant_owner(
                                &self.enums[enum_id].name,
                                &self.enums[enum_id].variants[variant as usize].name,
                            );
                            self.error(
                                *span,
                                format!("{owner} has named fields; use a named field pattern"),
                            );
                            return None;
                        }
                        let owner = variant_owner(
                            &self.enums[enum_id].name,
                            &self.enums[enum_id].variants[variant as usize].name,
                        );
                        let field_types = self.variant_field_types(
                            enum_id,
                            variant,
                            &application_value.arguments,
                        );
                        let fields = self.lower_positional_pattern(
                            elements,
                            *rest,
                            &field_types,
                            &owner,
                            *span,
                            ctx,
                        )?;
                        Some(hir::Pattern::Variant {
                            application,
                            variant,
                            fields,
                        })
                    }
                    PatternTarget::Struct(application) => {
                        let application_value = self.struct_applications[application].clone();
                        let struct_id = application_value.template;
                        let owner = format!("struct `{}`", self.structs[struct_id].name);
                        let declared: Vec<TypeId> = self.structs[struct_id]
                            .fields
                            .iter()
                            .map(|f| f.ty)
                            .collect();
                        let field_types: Vec<TypeId> = declared
                            .into_iter()
                            .map(|ty| self.instantiate_ty(ty, &application_value.arguments))
                            .collect();
                        let fields = self.lower_positional_pattern(
                            elements,
                            *rest,
                            &field_types,
                            &owner,
                            *span,
                            ctx,
                        )?;
                        Some(hir::Pattern::Struct {
                            application,
                            fields,
                        })
                    }
                }
            }
            ast::Pattern::Named {
                path,
                fields,
                rest,
                span,
            } => {
                let target = self.resolve_pattern_path(path, matched_ty, *span)?;
                match target {
                    PatternTarget::Variant(application, variant) => {
                        let application_value = self.enum_applications[application].clone();
                        let enum_id = application_value.template;
                        if !ctx.in_when {
                            self.error(
                                *span,
                                "refutable patterns are only allowed in `when`".to_string(),
                            );
                            return None;
                        }
                        let style = self.variant_styles[&(enum_id, variant)];
                        if matches!(style, VariantStyle::Unit | VariantStyle::Positional) {
                            let owner = variant_owner(
                                &self.enums[enum_id].name,
                                &self.enums[enum_id].variants[variant as usize].name,
                            );
                            self.error(
                                *span,
                                format!("{owner} has no named fields; use a positional pattern"),
                            );
                            return None;
                        }
                        let owner = variant_owner(
                            &self.enums[enum_id].name,
                            &self.enums[enum_id].variants[variant as usize].name,
                        );
                        let named_fields = self.variant_named_field_types(
                            enum_id,
                            variant,
                            &application_value.arguments,
                        );
                        let fields = self.lower_named_fields(
                            fields,
                            *rest,
                            &named_fields,
                            &owner,
                            *span,
                            ctx,
                        )?;
                        Some(hir::Pattern::Variant {
                            application,
                            variant,
                            fields,
                        })
                    }
                    PatternTarget::Struct(application) => {
                        let application_value = self.struct_applications[application].clone();
                        let struct_id = application_value.template;
                        let owner = format!("struct `{}`", self.structs[struct_id].name);
                        let declared: Vec<(String, TypeId)> = self.structs[struct_id]
                            .fields
                            .iter()
                            .map(|f| (f.name.clone(), f.ty))
                            .collect();
                        let named_fields: Vec<(String, TypeId)> = declared
                            .into_iter()
                            .map(|(name, ty)| {
                                (name, self.instantiate_ty(ty, &application_value.arguments))
                            })
                            .collect();
                        let fields = self.lower_named_fields(
                            fields,
                            *rest,
                            &named_fields,
                            &owner,
                            *span,
                            ctx,
                        )?;
                        Some(hir::Pattern::Struct {
                            application,
                            fields,
                        })
                    }
                }
            }
        }
    }

    /// A bare identifier that resolved to a variant of the matched
    /// enum: a unit variant matches as-is; a variant with fields must
    /// be written `V(...)`.
    fn bare_variant_pattern(
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

    /// Resolve a pattern's path (`V`, `E.V`, or none) against the
    /// matched type (spec 5.1: the enum prefix is optional, the
    /// variant resolves against the subject's type).
    fn resolve_pattern_path(
        &mut self,
        path: &[ast::Ident],
        matched_ty: TypeId,
        span: Span,
    ) -> Option<PatternTarget> {
        match path {
            // `S { f1, .. }` without a prefix: only structs (spec 5.3).
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
                Type::Struct(..) => {
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

    /// Positional patterns against a fixed field list (tuple elements,
    /// struct fields, or variant fields): arity and `..` rules (spec
    /// 4.6 — without `..` the arity must match exactly; with `..`
    /// leading elements bind to the first fields and trailing elements
    /// to the last fields). Returns `(field index, subpattern)` pairs
    /// in declaration order.
    fn lower_positional_pattern(
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
        // A `..` at most once is guaranteed by the AST shape
        // (`rest` is a single `Option`).
        if (rest.is_none() && count != total) || (rest.is_some() && count > total) {
            self.error(
                span,
                format!("pattern has {count} element(s), but {owner} has {total}"),
            );
            return None;
        }
        // Where does `..` sit? Elements before it bind to the first
        // fields, elements after it to the last fields. The AST carries
        // only the marker's span, so compare source positions.
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

    /// Named field patterns (`V { f1, f2: renamed, .. }`): every name
    /// must be a field of the owner, `_` is not allowed, duplicates
    /// are rejected, and an incomplete field list must end with `..`
    /// (spec 4.6 / 5.1). Each field binds a local (under `rename` when
    /// given). Returns pairs in declaration order.
    fn lower_named_fields(
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

    /// Declare a pattern binding in the current scope.
    fn bind_local(&mut self, name: &ast::Ident, ty: TypeId, mutable: bool) -> Option<hir::LocalId> {
        if self.scopes.is_declared_here(&name.text) {
            self.error(
                name.span,
                format!("`{}` is already declared in this scope", name.text),
            );
            return None;
        }
        let local = self.alloc_local(name.text.clone(), ty, mutable);
        self.scopes.declare(name.text.clone(), local);
        Some(local)
    }

    /// Variant field types with the enum's type parameters replaced by
    /// the matched type's arguments.
    fn variant_field_types(
        &mut self,
        enum_id: hir::EnumId,
        variant: u32,
        type_args: &[TypeId],
    ) -> Vec<TypeId> {
        let field_types: Vec<TypeId> = self.enums[enum_id].variants[variant as usize]
            .fields
            .iter()
            .map(|f| f.ty)
            .collect();
        field_types
            .into_iter()
            .map(|ty| self.instantiate_ty(ty, type_args))
            .collect()
    }

    /// Variant field names and types, instantiated (see
    /// `variant_field_types`).
    fn variant_named_field_types(
        &mut self,
        enum_id: hir::EnumId,
        variant: u32,
        type_args: &[TypeId],
    ) -> Vec<(String, TypeId)> {
        let fields: Vec<(String, TypeId)> = self.enums[enum_id].variants[variant as usize]
            .fields
            .iter()
            .map(|f| (f.name.clone(), f.ty))
            .collect();
        fields
            .into_iter()
            .map(|(name, ty)| (name, self.instantiate_ty(ty, type_args)))
            .collect()
    }

    /// Exhaustiveness (spec 5, milestone4 DESIGN.md 3.2):
    ///
    /// - an enum subject must cover every variant — only *unguarded*
    ///   variant patterns count (the compiler treats a guarded arm as
    ///   possibly not matching), and an unguarded binding or wildcard
    ///   arm covers everything — or provide an `else` branch;
    /// - a tuple or struct subject is exhaustive when an unguarded,
    ///   fully irrefutable arm exists (spec 5.2 / 5.3), or with `else`.
    pub(crate) fn check_exhaustiveness(
        &mut self,
        span: Span,
        subject_ty: TypeId,
        arms: &[hir::WhenArm],
        has_else: bool,
    ) {
        if has_else {
            return;
        }
        let Type::Enum(application) = self.types[subject_ty] else {
            let exhaustive = arms
                .iter()
                .any(|arm| arm.guard.is_none() && is_irrefutable(&arm.pattern));
            if !exhaustive {
                self.error(
                    span,
                    "non-exhaustive when: add a catch-all pattern or an `else` branch".to_string(),
                );
            }
            return;
        };
        let enum_id = self.enum_applications[application].template;
        let mut covered = HashSet::new();
        for arm in arms {
            if arm.guard.is_some() {
                continue;
            }
            match &arm.pattern {
                hir::Pattern::Binding { .. } | hir::Pattern::Wildcard => return,
                hir::Pattern::Variant { variant, .. } => {
                    covered.insert(*variant);
                }
                _ => {}
            }
        }
        let missing: Vec<String> = self.enums[enum_id]
            .variants
            .iter()
            .enumerate()
            .filter(|(index, _)| !covered.contains(&(*index as u32)))
            .map(|(_, variant)| format!("`{}`", variant.name))
            .collect();
        if !missing.is_empty() {
            self.error(
                span,
                format!(
                    "non-exhaustive when: missing variant(s) {}",
                    missing.join(", ")
                ),
            );
        }
    }
}

/// What a pattern path resolved to: an enum variant (with the matched
/// type's arguments, for instantiating field types) or a struct.
enum PatternTarget {
    Variant(hir::EnumApplicationId, u32),
    Struct(hir::StructApplicationId),
}

/// `variant \`V\` of \`E\``, for diagnostics.
fn variant_owner(enum_name: &str, variant_name: &str) -> String {
    format!("variant `{variant_name}` of `{enum_name}`")
}

/// Whether a pattern is irrefutable: binds or skips everything it
/// matches, so a single unguarded arm with it covers the whole type
/// (spec 5.2 / 5.3). Variant and literal patterns are refutable.
fn is_irrefutable(pattern: &hir::Pattern) -> bool {
    match pattern {
        hir::Pattern::Binding { .. } | hir::Pattern::Wildcard => true,
        hir::Pattern::Tuple(elements) => elements.iter().all(is_irrefutable),
        hir::Pattern::Struct { fields, .. } => fields.iter().all(|(_, sub)| is_irrefutable(sub)),
        hir::Pattern::Variant { .. } | hir::Pattern::Literal(_) => false,
    }
}

/// Whether an expression is a literal (patterns only match literals by
/// equality; a negative integer is unary minus over a literal).
fn is_literal_expr(expr: &ast::Expr) -> bool {
    match expr {
        ast::Expr::IntLiteral { .. }
        | ast::Expr::StringLiteral { .. }
        | ast::Expr::BoolLiteral { .. }
        | ast::Expr::UnitLiteral { .. } => true,
        ast::Expr::Unary {
            op: ast::UnOp::Neg,
            operand,
            ..
        } => matches!(&**operand, ast::Expr::IntLiteral { .. }),
        _ => false,
    }
}

/// The span of any pattern node (used to locate the `..` split point).
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
