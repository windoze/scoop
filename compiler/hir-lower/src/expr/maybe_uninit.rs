use super::*;

impl Lowerer {
    pub(in crate::expr) fn normalize_maybe_uninit(
        &self,
        kind: hir::MaybeUninitIntrinsic,
        receiver: Option<hir::Expr>,
        arguments: Vec<hir::Expr>,
        ty: TypeId,
        span: Span,
    ) -> hir::Expr {
        let operation = match kind {
            hir::MaybeUninitIntrinsic::Uninit => hir::MaybeUninitOperation::Uninit,
            hir::MaybeUninitIntrinsic::Initialized => {
                hir::MaybeUninitOperation::Initialized(Box::new(
                    arguments
                        .into_iter()
                        .next()
                        .expect("initialized has one checked argument"),
                ))
            }
            hir::MaybeUninitIntrinsic::AssumeInit => hir::MaybeUninitOperation::AssumeInit(
                Box::new(receiver.expect("assumeInit has its checked receiver")),
            ),
        };
        hir::Expr {
            kind: hir::ExprKind::MaybeUninit(operation),
            ty,
            span,
            origin: self.expression_origin(span),
        }
    }

    pub(crate) fn maybe_uninit_receiver(
        kind: hir::MaybeUninitIntrinsic,
        receiver: Option<hir::Expr>,
    ) -> Option<hir::Expr> {
        receiver.filter(|receiver| {
            kind == hir::MaybeUninitIntrinsic::AssumeInit
                || !matches!(receiver.kind, hir::ExprKind::SingletonValue(_))
        })
    }
}
