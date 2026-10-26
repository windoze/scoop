use super::*;

impl BodyLowerer<'_> {
    pub(super) fn lower_string_copy(
        &mut self,
        kind: hir::StringIntrinsic,
        args: &[hir::Expr],
        result: hir::TypeId,
    ) -> smir::Expr {
        use hir::StringIntrinsic as S;
        use mir::StringCopyOperation as Copy;
        let operation = match (kind, args) {
            (S::CopyUtf8Unchecked, [pointer, length]) => Copy::Utf8 {
                pointer: self.lower_expr(pointer),
                length: self.lower_expr(length),
            },
            (S::SliceBytes, [string, start, end]) => Copy::Slice {
                string: self.lower_expr(string),
                start: self.lower_expr(start),
                end: self.lower_expr(end),
            },
            (S::FromBytes, [bytes]) => Copy::FromBytes(self.lower_expr(bytes)),
            (S::ToBytes | S::ToMutableBytes, [string]) => Copy::ToBytes(self.lower_expr(string)),
            _ => unreachable!("the typed core String copy signature is complete"),
        };
        smir::Expr::new(
            self.lower_type(result),
            smir::ExprKind::StringCopy(Box::new(operation)),
        )
    }
}
