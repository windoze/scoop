//! Literal selection for the same closed numeric operations evaluated by const.

use super::*;

impl Lowerer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn probe_const_float_kind(
        &self,
        expression: &ast::Expr,
        expected: hir::FloatKind,
        file: usize,
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
        states: &[ConstState],
        stack: &[usize],
    ) -> Option<hir::FloatKind> {
        let mut probe = self.clone();
        let expected = probe.core_float_type(expected).ok()?;
        let value = probe.evaluate_const_expression(
            expression,
            Some(expected),
            file,
            declarations,
            ordinary,
            &mut states.to_vec(),
            &mut stack.to_vec(),
        )?;
        probe.float_kind(value.ty)
    }

    pub(super) fn select_const_float_binary_kind(
        &self,
        operator: ast::BinOp,
        receiver: &ast::Expr,
        expected: Option<hir::TypeId>,
        mut argument_accepts: impl FnMut(hir::FloatKind) -> bool,
    ) -> Option<hir::FloatKind> {
        let operation = float_binary_operator(operator)?;
        self.select_const_float_receiver_kind(receiver, expected, |_, kind, _| {
            argument_accepts(kind).then_some(!operation.is_predicate())
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn select_const_float_method_kind(
        &self,
        receiver: &ast::Expr,
        name: &str,
        args: &[ast::CallArgument],
        expected: Option<hir::TypeId>,
        file: usize,
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
        states: &[ConstState],
        stack: &[usize],
    ) -> Option<hir::FloatKind> {
        self.select_const_float_receiver_kind(receiver, expected, |probe, kind, ty| {
            let resolved = probe.resolve_const_float_intrinsic(ty, name)?;
            if !resolved.arguments_match(args) {
                return None;
            }
            if let [argument] = args
                && probe.probe_const_float_kind(
                    &argument.expression,
                    kind,
                    file,
                    declarations,
                    ordinary,
                    states,
                    stack,
                ) != Some(kind)
            {
                return None;
            }
            Some(match resolved.kind {
                hir::FloatIntrinsicKind::Unary { operation, .. } => !operation.is_predicate(),
                hir::FloatIntrinsicKind::Binary { operation, .. } => !operation.is_predicate(),
                hir::FloatIntrinsicKind::Conversion(_) => false,
            })
        })
    }

    fn select_const_float_receiver_kind(
        &self,
        receiver: &ast::Expr,
        expected: Option<hir::TypeId>,
        mut applicable: impl FnMut(&mut Lowerer, hir::FloatKind, hir::TypeId) -> Option<bool>,
    ) -> Option<hir::FloatKind> {
        let kinds = crate::expr::float_literal_candidate_kinds(receiver)?;
        let expected = expected.and_then(|ty| self.float_kind(ty));
        let mut successful = Vec::new();
        for &kind in kinds {
            let mut probe = self.clone();
            let Ok(ty) = probe.core_float_type(kind) else {
                continue;
            };
            if probe
                .lower_expr(receiver, &mut Vec::new(), Some(ty))
                .is_none()
            {
                continue;
            }
            if let Some(preserves) = applicable(&mut probe, kind, ty) {
                successful.push((kind, preserves));
            }
        }
        expected
            .and_then(|expected| {
                successful
                    .iter()
                    .find(|&&(kind, preserves)| kind == expected && preserves)
            })
            .or_else(|| successful.iter().find(|&&(kind, _)| kind == kinds[0]))
            .or_else(|| (successful.len() == 1).then(|| &successful[0]))
            .map(|&(kind, _)| kind)
    }
}
