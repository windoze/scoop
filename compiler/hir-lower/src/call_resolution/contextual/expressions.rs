use scoop_ast as ast;
use scoop_hir as hir;

use crate::Lowerer;

#[derive(Clone, Copy)]
pub(crate) enum ArgumentExpression<'a> {
    Source(&'a ast::Expr),
    Lowered(&'a hir::Expr),
    Addressable {
        place: hir::Place,
        ty: hir::TypeId,
        span: ast::Span,
    },
}

pub(crate) struct ArgumentExpressionFailure {
    pub(crate) source_index: usize,
    pub(crate) expected: Option<hir::TypeId>,
    pub(crate) span: ast::Span,
    pub(crate) reason: String,
}

pub(super) struct ArgumentAttempt {
    pub(super) state: Box<Lowerer>,
    pub(super) result: Result<(hir::Expr, Vec<hir::Statement>), ArgumentExpressionFailure>,
}

impl ArgumentExpression<'_> {
    /// Keep the complete candidate copy outside recursive contextual typing
    /// frames; only a default-seed attempt needs to own a rollback state.
    pub(super) fn try_default(self, state: &Lowerer, source_index: usize) -> ArgumentAttempt {
        let mut attempt = Box::new(state.clone());
        let result = self.lower(&mut attempt, source_index, None);
        ArgumentAttempt {
            state: attempt,
            result,
        }
    }

    pub(super) fn requires_context(self, state: &Lowerer) -> bool {
        match self {
            Self::Source(expression) => state.expr_requires_expected_type(expression),
            Self::Lowered(_) | Self::Addressable { .. } => false,
        }
    }

    pub(super) fn seed_rank(self, state: &Lowerer) -> Option<(u32, hir::IntegerSignedness)> {
        match self {
            Self::Source(expression) => state.expr_default_seed_rank(expression),
            Self::Lowered(_) | Self::Addressable { .. } => None,
        }
    }

    pub(super) fn lower(
        self,
        state: &mut Lowerer,
        source_index: usize,
        expected: Option<hir::TypeId>,
    ) -> Result<(hir::Expr, Vec<hir::Statement>), ArgumentExpressionFailure> {
        let before = state.diagnostics.len();
        let mut sink = Vec::new();
        let (value, span) = match self {
            Self::Source(expression) => (
                state.lower_expr(expression, &mut sink, expected),
                expression.span(),
            ),
            Self::Lowered(expression) => (Some(expression.clone()), expression.span),
            Self::Addressable { place, ty, span } => (
                Some(hir::Expr {
                    kind: match place {
                        hir::Place::Local(local) => hir::ExprKind::Local(local),
                        hir::Place::Global(global) => hir::ExprKind::GlobalRead(global),
                    },
                    ty,
                    span,
                    origin: state.expression_origin(span),
                }),
                span,
            ),
        };
        if let Some(value) = value
            && state.diagnostics.len() == before
        {
            return Ok((value, sink));
        }
        let diagnostics = &state.diagnostics[before..];
        let reason = if diagnostics.is_empty() {
            "expression could not be typed".to_owned()
        } else {
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        Err(ArgumentExpressionFailure {
            source_index,
            expected,
            span: diagnostics
                .first()
                .and_then(|diagnostic| diagnostic.span)
                .unwrap_or(span),
            reason,
        })
    }
}
