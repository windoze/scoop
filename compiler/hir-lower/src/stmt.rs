//! Statement lowering: declarations, assignments, control flow,
//! `return` and block-level scoping (milestone2 DESIGN.md 2.2,
//! milestone3 DESIGN.md 2.2).
//!
//! M4 (milestone4 DESIGN.md 3.2): statement-level `when` with pattern
//! arms, guards and exhaustiveness checking, and destructuring
//! `val` / `var` declarations (the binding target is a pattern, not
//! just an identifier).
//!
//! M5 (milestone5 DESIGN.md 2.2): subscript assignment
//! (`array[index] = value`) alongside local assignment.
//!
//! M8 (milestone8 DESIGN.md 3.2): `throw` and
//! `try { } catch (e: T) { } finally { }`.

use scoop_ast as ast;
use scoop_hir as hir;

use ast::Span;
use hir::{FunctionId, Type, TypeId};

use crate::patterns::PatternCtx;
use crate::scope::Scopes;
use crate::{CaptureContext, FnSig, ForbiddenSuspendContext, Lowerer, SuspensionContext};

pub(crate) struct ValueBlock {
    pub(crate) statements: Vec<hir::Statement>,
    /// `None` means the block has no normally completing path.
    pub(crate) value: Option<hir::Expr>,
}

impl ValueBlock {
    /// Preserve evaluation when this block does not contribute a structured
    /// expression result. Unit literals have no observable effect.
    fn discard_value(&mut self) {
        if let Some(value) = self.value.take()
            && !matches!(value.kind, hir::ExprKind::UnitLiteral)
        {
            self.statements.push(hir::Statement {
                span: value.span,
                kind: hir::StatementKind::Expr(value),
            });
        }
    }
}

mod assignments;
mod blocks;
mod body;
mod compound_assignments;
mod exceptions;
mod flow;
mod local_functions;
mod places;
mod statements;
mod when;

pub(crate) use flow::statements_control_outcomes;
