use super::*;

impl BodyLowerer<'_> {
    pub(super) fn lower_maybe_uninit(
        &mut self,
        kind: hir::MaybeUninitIntrinsic,
        operand: Option<&hir::Expr>,
        result: hir::TypeId,
    ) -> smir::Expr {
        let result = self.lower_type(result);
        let operation = match kind {
            hir::MaybeUninitIntrinsic::Uninit => mir::MaybeUninitOperation::Uninit,
            hir::MaybeUninitIntrinsic::Initialized => mir::MaybeUninitOperation::Initialized(
                Box::new(self.lower_expr(operand.expect("initialized has one checked value"))),
            ),
            hir::MaybeUninitIntrinsic::AssumeInit => mir::MaybeUninitOperation::AssumeInit(
                Box::new(self.lower_expr(operand.expect("assumeInit has its checked receiver"))),
            ),
        };
        maybe_uninit_value(result, operation)
    }
}

pub(crate) fn maybe_uninit_value(
    result: mir::Type,
    operation: mir::MaybeUninitOperation<Box<smir::Expr>>,
) -> smir::Expr {
    let wrapper = match &operation {
        mir::MaybeUninitOperation::AssumeInit(value) => &value.ty,
        _ => &result,
    };
    let mir::Type::Struct(wrapper) = wrapper else {
        unreachable!("MaybeUninit operations preserve their exact intrinsic struct")
    };
    let wrapper = *wrapper;
    smir::Expr::new(result, smir::ExprKind::MaybeUninit { wrapper, operation })
}
