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

mod binding;
mod exhaustiveness;
mod structure;
mod variants;

pub(crate) use binding::BindingSubject;
use structure::pattern_span;

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
        self.with_pattern_transaction(|state| state.lower_pattern_inner(pattern, matched_ty, ctx))
    }

    /// Run one complete pattern or binding owner against private lowering
    /// state. Recursive helpers call their non-transactional inner forms, so
    /// each owner clones at most once.
    pub(super) fn with_pattern_transaction<T>(
        &mut self,
        build: impl FnOnce(&mut Lowerer) -> Option<T>,
    ) -> Option<T> {
        let diagnostics_before = self.diagnostics.len();
        let mut state = self.clone();
        let lowered = build(&mut state);
        if let Some(value) = lowered
            && state.diagnostics.len() == diagnostics_before
        {
            *self = state;
            return Some(value);
        }

        // A dependency such as a previously failed type alias may own the only
        // Error. Failure without a new Error still rolls back; warnings stay
        // private unless the whole owner commits successfully.
        self.diagnostics
            .extend(state.diagnostics.into_iter().skip(diagnostics_before));
        None
    }

    pub(super) fn lower_pattern_inner(
        &mut self,
        pattern: &ast::Pattern,
        matched_ty: TypeId,
        ctx: PatternCtx,
    ) -> Option<hir::Pattern> {
        match pattern {
            ast::Pattern::Binding(name) => {
                // Variant-first bare-name lookup belongs exclusively to
                // match patterns. In binding positions every ordinary bare
                // identifier introduces a new local, even when the subject
                // enum has a variant with the same name. `Unit` / `()` reach
                // this stage as literal patterns and remain rejected below.
                let unmatched_enum = if ctx.in_when {
                    if matches!(self.types[matched_ty], Type::Enum(_)) {
                        if let Some(application) = self.named_enum_variant(matched_ty, &name.text) {
                            return self.bare_variant_pattern(name, application, ctx);
                        }
                        Some(self.type_name(matched_ty))
                    } else {
                        None
                    }
                } else {
                    None
                };
                let local = self.bind_local(name, matched_ty, ctx.mutable)?;
                if ctx.in_when
                    && let Some(enum_name) = unmatched_enum
                {
                    self.warning(
                        name.span,
                        format!(
                            "`{}` is a catch-all binding because `{enum_name}` has no variant named `{}`; qualify an intended variant as `E.V`, or use `_` or an intentional binding name for a catch-all",
                            name.text, name.text
                        ),
                    );
                }
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
                let prefixed_integer_literal = match &**expr {
                    ast::Expr::Unary { op, operand, .. } => match &**operand {
                        ast::Expr::IntLiteral(literal)
                            if *op == ast::UnOp::Neg
                                && matches!(
                                    literal.suffix,
                                    ast::IntegerSuffix::Unsigned | ast::IntegerSuffix::UnsignedLong
                                ) =>
                        {
                            Some((*op, *literal))
                        }
                        _ => None,
                    },
                    _ => None,
                };
                let literal = if let Some((op, source)) = prefixed_integer_literal {
                    let mut value =
                        self.lower_integer_literal(source, Some(matched_ty), false, *span)?;
                    let Type::Integer(kind) = self.types[value.ty] else {
                        unreachable!("an integer literal has an integer type")
                    };
                    debug_assert_eq!(op, ast::UnOp::Neg);
                    let operation = hir::NoGcIntegerOperation::UnaryMinus;
                    let intrinsic = hir::IntegerIntrinsicKind::NoGcOperation { kind, operation };
                    if !self.const_integer_operation_available(intrinsic) {
                        self.error(
                            *span,
                            format!(
                                "type `{}` has no typed core `unaryMinus` intrinsic",
                                kind.canonical_name(),
                            ),
                        );
                        return None;
                    }
                    let hir::ExprKind::IntegerLiteral(constant) = value.kind else {
                        unreachable!("literal lowering produces an integer constant")
                    };
                    value.kind = hir::ExprKind::IntegerLiteral(
                        crate::globals::integer_wrapping_neg(constant),
                    );
                    value
                } else {
                    let mut sink = Vec::new();
                    self.lower_expr(expr, &mut sink, Some(matched_ty))?
                };
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
                let (literal, equality) =
                    self.resolve_literal_pattern_equality(matched_ty, literal, *span)?;
                Some(hir::Pattern::Literal {
                    value: literal,
                    equality,
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
                Type::Struct(_) => {
                    self.lower_struct_positional_pattern(matched_ty, elements, *rest, *span, ctx)
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
                    PatternTarget::Variant(application) => self.lower_variant_positional_pattern(
                        application,
                        elements,
                        *rest,
                        *span,
                        ctx,
                    ),
                    PatternTarget::Struct(owner) => {
                        self.lower_struct_positional_pattern(owner, elements, *rest, *span, ctx)
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
                    PatternTarget::Variant(application) => {
                        self.lower_variant_named_pattern(application, fields, *rest, *span, ctx)
                    }
                    PatternTarget::Struct(owner) => {
                        self.lower_struct_named_pattern(owner, fields, *rest, *span, ctx)
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
        let local = self.alloc_declared_local(name.text.clone(), ty, mutable, name.span);
        self.scopes.declare(name.text.clone(), local);
        Some(local)
    }
}

/// What a pattern path resolved to: an enum variant (with the matched
/// type's arguments, for instantiating field types) or a struct.
enum PatternTarget {
    Variant(hir::EnumVariantApplication),
    Struct(TypeId),
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
        ast::Expr::IntLiteral(_)
        | ast::Expr::StringLiteral { .. }
        | ast::Expr::BoolLiteral { .. }
        | ast::Expr::UnitLiteral { .. } => true,
        ast::Expr::Unary {
            op: ast::UnOp::Neg,
            operand,
            ..
        } => {
            matches!(&**operand, ast::Expr::IntLiteral(_))
        }
        _ => false,
    }
}
