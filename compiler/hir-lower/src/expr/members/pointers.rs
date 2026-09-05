use super::*;

impl Lowerer {
    pub(in crate::expr) fn normalize_pointer_method_call(
        &self,
        function: hir::FunctionId,
        receiver: hir::Expr,
        args: Vec<hir::Expr>,
        ty: TypeId,
        span: Span,
    ) -> Option<hir::Expr> {
        let hir::FunctionKind::Intrinsic(intrinsic) = self.functions[function].kind else {
            return None;
        };
        let hir::IntrinsicFunctionKind::Pointer(kind) = intrinsic.kind else {
            return None;
        };
        if matches!(
            kind,
            hir::PointerIntrinsic::AddressOf
                | hir::PointerIntrinsic::SizeOf
                | hir::PointerIntrinsic::AlignOf
        ) {
            return None;
        }
        let mut args = args.into_iter();
        let pointer = Box::new(receiver);
        let expr = match kind {
            hir::PointerIntrinsic::ToULong => ExprKind::PtrToULong(pointer),
            hir::PointerIntrinsic::Cast => ExprKind::PtrCast(pointer),
            hir::PointerIntrinsic::Load => ExprKind::PtrLoad {
                pointer,
                offset: None,
            },
            hir::PointerIntrinsic::LoadOffset => ExprKind::PtrLoad {
                pointer,
                offset: Some(Box::new(args.next().expect("validated offset argument"))),
            },
            hir::PointerIntrinsic::Store => ExprKind::PtrStore {
                pointer,
                offset: None,
                value: Box::new(args.next().expect("validated store value")),
            },
            hir::PointerIntrinsic::StoreOffset => ExprKind::PtrStore {
                pointer,
                offset: Some(Box::new(args.next().expect("validated offset argument"))),
                value: Box::new(args.next().expect("validated store value")),
            },
            hir::PointerIntrinsic::Plus | hir::PointerIntrinsic::Minus => ExprKind::PtrOffset {
                pointer,
                offset: Box::new(args.next().expect("validated pointer offset")),
                subtract: kind == hir::PointerIntrinsic::Minus,
            },
            _ => unreachable!("top-level pointer intrinsic is not a method"),
        };
        Some(hir::Expr {
            kind: expr,
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }
}
