use super::*;

impl Lowerer {
    pub(super) fn commit_imported_intrinsic(
        &mut self,
        operation: ImportedIntrinsicCall,
        receiver: Option<hir::Expr>,
        arguments: Vec<hir::Expr>,
        result_type: hir::TypeId,
        span: ast::Span,
        sink: &[hir::Statement],
    ) -> Option<hir::Expr> {
        match operation {
            ImportedIntrinsicCall::MaybeUninit(kind) => {
                Some(self.normalize_maybe_uninit(kind, receiver, arguments, result_type, span))
            }
            ImportedIntrinsicCall::Atomic(kind) => self.normalize_atomic_method(
                kind,
                receiver.expect("an atomic member has a receiver"),
                arguments,
                result_type,
                span,
                sink,
            ),
            ImportedIntrinsicCall::ForeignCallback { kind, native_type } => {
                use hir::IntrinsicFunctionKind as Kind;
                let operation = match kind {
                    Kind::ForeignCallbackRegister => {
                        return self.normalize_foreign_callback_registration(
                            native_type,
                            arguments,
                            result_type,
                            span,
                            sink,
                        );
                    }
                    Kind::ForeignCallbackRetain => hir::ForeignCallbackOperation::Retain,
                    Kind::ForeignCallbackRelease => hir::ForeignCallbackOperation::Release,
                    Kind::ForeignCallbackState => hir::ForeignCallbackOperation::State,
                    Kind::ForeignCallbackFailure => hir::ForeignCallbackOperation::Failure,
                    _ => unreachable!("the selected operation is a callback intrinsic"),
                };
                self.normalize_foreign_callback_operation(
                    operation,
                    native_type,
                    arguments,
                    result_type,
                    span,
                )
            }
            ImportedIntrinsicCall::PointerMember(intrinsic) => {
                Some(self.normalize_pointer_intrinsic(
                    intrinsic,
                    receiver.expect("a pointer intrinsic has a receiver"),
                    arguments,
                    result_type,
                    span,
                ))
            }
            ImportedIntrinsicCall::ArrayConversion(intrinsic) => self
                .normalize_array_intrinsic_call(
                    hir::IntrinsicFunctionKind::Array(intrinsic),
                    receiver.expect("an array intrinsic has a receiver"),
                    &arguments,
                    result_type,
                    span,
                ),
            ImportedIntrinsicCall::ArrayAccess(intrinsic) => self.normalize_array_intrinsic_call(
                hir::IntrinsicFunctionKind::ArrayAccess(intrinsic),
                receiver.expect("an array intrinsic has a receiver"),
                &arguments,
                result_type,
                span,
            ),
            ImportedIntrinsicCall::Expression(_) => {
                unreachable!("expression intrinsics precede argument materialization")
            }
        }
    }
}
