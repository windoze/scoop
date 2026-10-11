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
