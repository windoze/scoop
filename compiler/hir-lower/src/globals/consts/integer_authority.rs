use scoop_ast as ast;
use scoop_hir as hir;

use crate::Lowerer;

impl Lowerer {
    pub(in crate::globals) fn const_integer_operation_available(
        &self,
        kind: hir::IntegerIntrinsicKind,
    ) -> bool {
        if let hir::IntegerIntrinsicKind::NoGcOperation { kind, operation } = kind
            && !operation.supports(kind)
        {
            return false;
        }
        let key = hir::IntrinsicFunctionKind::Integer(kind);
        let Some(&(function, _)) = self.intrinsic_functions.get(&key) else {
            return self.dependencies.as_ref().is_some_and(|dependencies| {
                dependencies.has_intrinsic_callable(
                    key,
                    match kind.gc_effect() {
                        hir::GcEffect::NoGc => scoop_identity::GcEffect::NoGc,
                        hir::GcEffect::Managed => scoop_identity::GcEffect::Managed,
                    },
                )
            });
        };
        match kind {
            hir::IntegerIntrinsicKind::ManagedOperation { .. } => {
                hir::ManagedCallableRef::try_from_function(function, &self.functions).is_some()
            }
            hir::IntegerIntrinsicKind::NoGcOperation { .. }
            | hir::IntegerIntrinsicKind::Conversion { .. } => {
                hir::NoGcCallableRef::try_from_function(function, &self.functions).is_some()
            }
        }
    }

    pub(in crate::globals) fn select_const_binary_literal_kind(
        &self,
        operator: ast::BinOp,
        receiver: &ast::Expr,
        expected: Option<hir::TypeId>,
        mut argument_accepts: impl FnMut(hir::IntegerKind) -> bool,
    ) -> Option<hir::IntegerKind> {
        let mut applicable = crate::expr::integer_literal_candidate_kinds(receiver)?
            .into_iter()
            .filter(|&kind| {
                binary_operation(operator, kind).is_some_and(|operation| {
                    self.const_integer_operation_available(operation) && argument_accepts(kind)
                })
            })
            .collect::<Vec<_>>();
        let preserves_kind = matches!(
            operator,
            ast::BinOp::Add | ast::BinOp::Sub | ast::BinOp::Mul | ast::BinOp::Div | ast::BinOp::Rem
        );
        let expected_kind = expected.and_then(|expected| match self.types[expected] {
            hir::Type::Integer(kind) if preserves_kind => Some(kind),
            _ => None,
        });
        let preferred = expected_kind
            .or_else(|| crate::expr::integer_literal_default_kind(receiver))
            .and_then(|kind| applicable.iter().position(|candidate| *candidate == kind))
            .or_else(|| (applicable.len() == 1).then_some(0))?;
        Some(applicable.swap_remove(preferred))
    }
}

fn binary_operation(
    operator: ast::BinOp,
    kind: hir::IntegerKind,
) -> Option<hir::IntegerIntrinsicKind> {
    use hir::NoGcIntegerOperation as Operation;
    let operation = match operator {
        ast::BinOp::Add => Operation::Add,
        ast::BinOp::Sub => Operation::Sub,
        ast::BinOp::Mul => Operation::Mul,
        ast::BinOp::Eq | ast::BinOp::Ne => Operation::Equals,
        ast::BinOp::Lt | ast::BinOp::Le | ast::BinOp::Gt | ast::BinOp::Ge => Operation::CompareTo,
        ast::BinOp::Div | ast::BinOp::Rem => {
            return Some(hir::IntegerIntrinsicKind::ManagedOperation {
                kind,
                operation: if operator == ast::BinOp::Div {
                    hir::IntegerDivRem::Div
                } else {
                    hir::IntegerDivRem::Rem
                },
            });
        }
        _ => return None,
    };
    Some(hir::IntegerIntrinsicKind::NoGcOperation { kind, operation })
}
