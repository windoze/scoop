//! Pattern lowering for `when` arms and destructuring declarations
//! (spec 4.6 / 5, milestone22 DESIGN.md 4.4).
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
//!   is proved over the whole arm list (`prove_exhaustiveness`);
//! - `val` / `var` declarations (`in_when: false`): only irrefutable
//!   patterns (bindings, wildcards, tuple and struct patterns) are
//!   allowed — an enum variant or literal pattern is diagnosed with
//!   "refutable patterns are only allowed in `when`".
//!
//! The `..` rest marker's *position* is not explicit in the AST
//! (`rest` is just its span); the split between leading and trailing
//! elements is recovered by comparing element spans against it (the
//! parser assigns spans in source order).

use scoop_ast as ast;
use scoop_hir as hir;

use ast::Span;
use hir::{Type, TypeId};

use crate::{Lowerer, VariantStyle};

mod exhaustiveness;
mod structure;

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
                // `Unit` has exactly one constructor. Normalize its literal
                // pattern to the same unconditional HIR form as `_`; this
                // avoids inventing an equality call for a one-value type and
                // lets the matrix checker treat `()` as exhaustive.
                if matches!(self.types[matched_ty], Type::Unit)
                    && matches!(literal.kind, hir::ExprKind::UnitLiteral)
                {
                    return Some(hir::Pattern::Wildcard);
                }
                let (literal, equals) =
                    self.resolve_literal_pattern_equality(matched_ty, literal, *span)?;
                Some(hir::Pattern::Literal {
                    value: literal,
                    equals,
                    subject_ty: matched_ty,
                })
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
                        .semantic_fields()
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
                            .semantic_fields()
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
                            .semantic_fields()
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
pub(super) fn is_irrefutable(pattern: &hir::Pattern) -> bool {
    match pattern {
        hir::Pattern::Binding { .. } | hir::Pattern::Wildcard => true,
        hir::Pattern::Tuple(elements) => elements.iter().all(is_irrefutable),
        hir::Pattern::Struct { fields, .. } => fields.iter().all(|(_, sub)| is_irrefutable(sub)),
        hir::Pattern::Variant { .. } | hir::Pattern::Literal { .. } => false,
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
