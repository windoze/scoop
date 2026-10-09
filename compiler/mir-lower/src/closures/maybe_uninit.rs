use super::*;

pub(super) fn reference_intrinsic(
    module: &hir::Module,
    callable: Option<hir::CallableTarget>,
) -> Option<hir::MaybeUninitIntrinsic> {
    let hir::CallableTarget::Local(hir::Callable::Function(function)) = callable? else {
        return None;
    };
    match &module.functions[function].kind {
        hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
            kind: hir::IntrinsicFunctionKind::MaybeUninit(kind),
            ..
        }) => Some(*kind),
        _ => None,
    }
}

pub(super) fn reference_value(
    kind: hir::MaybeUninitIntrinsic,
    mut args: Vec<smir::Expr>,
    result: mir::Type,
) -> smir::Expr {
    let operation = match kind {
        hir::MaybeUninitIntrinsic::Uninit => mir::MaybeUninitOperation::Uninit,
        hir::MaybeUninitIntrinsic::Initialized => mir::MaybeUninitOperation::Initialized(Box::new(
            args.pop()
                .expect("initialized has one checked value after its receiver"),
        )),
        hir::MaybeUninitIntrinsic::AssumeInit => mir::MaybeUninitOperation::AssumeInit(Box::new(
            args.into_iter()
                .next()
                .expect("assumeInit has its checked receiver"),
        )),
    };
    crate::body::maybe_uninit_value(result, operation)
}
