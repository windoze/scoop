use scoop_ast as ast;
use scoop_hir as hir;

use super::{ConstIntegerIntrinsicKind, ConstState};

mod infix;
mod method;
use crate::Lowerer;
use crate::globals::{PendingConst, PendingOrdinary};

impl Lowerer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn probe_const_integer_kind(
        &self,
        expression: &ast::Expr,
        expected: Option<hir::IntegerKind>,
        file: usize,
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
        states: &[ConstState],
        stack: &[usize],
    ) -> Option<hir::IntegerKind> {
        let mut probe = self.clone();
        let expected = expected.map(|kind| probe.integer_type(kind));
        let mut probe_states = states.to_vec();
        let mut probe_stack = stack.to_vec();
        let evaluated = probe.evaluate_const_expression(
            expression,
            expected,
            file,
            declarations,
            ordinary,
            &mut probe_states,
            &mut probe_stack,
        )?;
        let hir::ConstPropertyValue::Integer(value) = evaluated.value else {
            return None;
        };
        matches!(probe.types[evaluated.ty], hir::Type::Integer(kind) if kind == value.kind())
            .then_some(value.kind())
    }

    pub(in crate::globals) fn const_integer_intrinsic_argument_kind(
        &self,
        source: hir::IntegerKind,
        source_name: &str,
    ) -> Option<hir::IntegerKind> {
        match self
            .resolve_const_integer_intrinsic(source, source_name)?
            .kind
        {
            ConstIntegerIntrinsicKind::NoGcOperation(operation)
                if operation.arity() == hir::IntegerOperationArity::Binary =>
            {
                Some(
                    if matches!(
                        operation,
                        hir::NoGcIntegerOperation::Shl
                            | hir::NoGcIntegerOperation::Shr
                            | hir::NoGcIntegerOperation::Ushr
                    ) {
                        hir::IntegerKind::SIGNED_64
                    } else {
                        source
                    },
                )
            }
            ConstIntegerIntrinsicKind::ManagedDivRem(_) => Some(source),
            ConstIntegerIntrinsicKind::NoGcOperation(_)
            | ConstIntegerIntrinsicKind::Conversion(_) => None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn select_const_integer_literal_receiver_kind(
        &self,
        receiver: &ast::Expr,
        argument: Option<&ast::Expr>,
        source_name: &str,
        expected: Option<hir::TypeId>,
        require_infix: bool,
        file: usize,
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
        states: &[ConstState],
        stack: &[usize],
    ) -> Option<hir::IntegerKind> {
        let candidates = crate::expr::integer_literal_candidate_kinds(receiver)?;
        let mut applicable = candidates
            .into_iter()
            .filter(|&kind| {
                let Some(resolved) = self.resolve_const_integer_intrinsic(kind, source_name) else {
                    return false;
                };
                if require_infix && !resolved.is_infix {
                    return false;
                }
                match (
                    self.const_integer_intrinsic_argument_kind(kind, source_name),
                    argument,
                ) {
                    (Some(argument_kind), Some(argument)) => {
                        self.probe_const_integer_kind(
                            argument,
                            Some(argument_kind),
                            file,
                            declarations,
                            ordinary,
                            states,
                            stack,
                        ) == Some(argument_kind)
                    }
                    (None, None) => true,
                    (Some(_), None) | (None, Some(_)) => false,
                }
            })
            .collect::<Vec<_>>();
        let expected_kind = expected.and_then(|expected| match self.types[expected] {
            hir::Type::Integer(kind)
                if self.const_integer_receiver_expected(source_name, Some(expected))
                    == Some(expected) =>
            {
                Some(kind)
            }
            _ => None,
        });
        let preferred = expected_kind
            .or_else(|| crate::expr::integer_literal_default_kind(receiver))
            .and_then(|kind| applicable.iter().position(|candidate| *candidate == kind))
            .or_else(|| (applicable.len() == 1).then_some(0))?;
        Some(applicable.swap_remove(preferred))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn select_const_equality_integer_kind(
        &self,
        left: &ast::Expr,
        right: &ast::Expr,
        file: usize,
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
        states: &[ConstState],
        stack: &[usize],
    ) -> Option<hir::IntegerKind> {
        if let Some(kind) = crate::expr::common_integer_literal_kind(&[left, right]) {
            return Some(kind);
        }
        if crate::expr::integer_literal_candidate_kinds(left).is_some() {
            let right_kind = self.probe_const_integer_kind(
                right,
                None,
                file,
                declarations,
                ordinary,
                states,
                stack,
            )?;
            return crate::expr::integer_literal_accepts_kind(left, right_kind)
                .then_some(right_kind);
        }
        if crate::expr::integer_literal_candidate_kinds(right).is_some() {
            let left_kind = self.probe_const_integer_kind(
                left,
                None,
                file,
                declarations,
                ordinary,
                states,
                stack,
            )?;
            return crate::expr::integer_literal_accepts_kind(right, left_kind)
                .then_some(left_kind);
        }
        None
    }
}
