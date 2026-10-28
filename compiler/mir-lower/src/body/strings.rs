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
            (S::AllocateZeroed, [length]) => Copy::Zeroed(self.lower_expr(length)),
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
            (S::FromBytePrefixUnchecked, [array, length]) => Copy::FromBytePrefix {
                array: self.lower_expr(array),
                length: self.lower_expr(length),
            },
            (S::CopyToBytesUnchecked, [string, target, start]) => Copy::StringIntoBytes {
                string: self.lower_expr(string),
                target: self.lower_expr(target),
                start: self.lower_expr(start),
            },
            (S::CopyBytePrefixUnchecked, [source, target, length]) => Copy::BytePrefixIntoBytes {
                source: self.lower_expr(source),
                target: self.lower_expr(target),
                length: self.lower_expr(length),
            },
            (S::CopyRawBytesUnchecked, [pointer, length, target, start]) => Copy::RawIntoBytes {
                pointer: self.lower_expr(pointer),
                length: self.lower_expr(length),
                target: self.lower_expr(target),
                start: self.lower_expr(start),
            },
            _ => unreachable!("the typed core String copy signature is complete"),
        };
        smir::Expr::new(
            self.lower_type(result),
            smir::ExprKind::StringCopy(Box::new(operation)),
        )
    }

    pub(super) fn lower_string_byte(
        &mut self,
        kind: hir::StringIntrinsic,
        args: &[hir::Expr],
        result: hir::TypeId,
    ) -> smir::Expr {
        use mir::StringByteOperation as Byte;
        let operation = match (kind, args) {
            (hir::StringIntrinsic::ByteAtUnchecked, [string, index]) => Byte::Read {
                string: self.lower_expr(string),
                index: self.lower_expr(index),
            },
            (hir::StringIntrinsic::SetByteUnchecked, [string, index, value]) => Byte::Write {
                string: self.lower_expr(string),
                index: self.lower_expr(index),
                value: self.lower_expr(value),
            },
            _ => unreachable!("the typed String byte signature is complete"),
        };
        smir::Expr::new(
            self.lower_type(result),
            smir::ExprKind::StringByte(Box::new(operation)),
        )
    }
}
